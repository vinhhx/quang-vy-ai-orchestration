# Plan: bước triage của hàng chờ dùng engine chọn được trong Cài đặt, và báo rõ lý do khi lỗi

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main` (afeab0d trở lên). Đọc toàn bộ file này trước khi làm.

## Sự cố (2026-11-06 13:20, intake #1 từ Telegram, dự án `srt`)
- `orchestrator/triage.rs::triage_with_warnings` (~dòng 340–385) luôn dùng `Engine::Claude` với profile của dự án (`srt` → `personal` → `claude_config_dir: ~/.claude-orca`). Tài khoản đó hết hạn đăng nhập; stream có `"error":"authentication_failed"` và `result`: "Failed to authenticate: this login reached its time limit…".
- agent-brain chỉ ghi `intake.error = "Engine triage thất bại với trạng thái failed"`, không nói lý do; người dùng không biết phải làm gì.
- Đội hình (`teams.json`) và engine (`claude-max`, `agy`, `deepseek`) người dùng đã cấu hình không được dùng cho triage.
- Gỡ tạm đã làm (không thuộc plan): đổi `profiles.personal.claude_config_dir` sang `~/.claude-max`.

## Mục tiêu
1. Thêm `orchestrator.triage_engine` (chuỗi, mặc định `"claude"`) vào `OrchestratorConfig` (`config.rs`), kiểm hợp lệ: `"claude"` hoặc id một engine trong `engines` có `kind` `anthropic` hoặc `agy` (dsh/human không hỗ trợ cho triage → lỗi config rõ ràng). Thêm vào trạng thái sống `AppState.orch` và API `GET/POST /api/orchestrator/settings` (trường mới, kiểm như trên; giữ tương thích: POST thiếu trường này thì giữ giá trị cũ).
2. `triage.rs`: dựng `ResolvedEngine` từ `triage_engine` (`Engine::Claude` hoặc `Engine::Named(id)`), đi qua đúng `engine::plan_for` như Lead (engine `anthropic` có `claude_config_dir` riêng thì dùng của engine; profile dự án vẫn quyết định env như hiện nay). Với engine `agy`: chạy chế độ chỉ đọc như Lead agy (không cờ quyền), dùng `finish_after_result: Some(AGY_AFTER_RESULT_GRACE)` như role agy, và lấy JSON đề xuất từ `result.response` (Summary đã có `agy_response`); thêm vào danh sách ứng viên parse.
3. Lý do lỗi rõ ràng: khi run triage không thành công hoặc không parse được, `intake.error` có dạng `Triage (<engine>) lỗi: <lý do>` với lý do lấy theo thứ tự:
   - stream Claude có `error` (ví dụ `authentication_failed`) hoặc `result` là thông báo lỗi khi `is_error: true` → dùng dòng đầu của `result` (cắt 300 ký tự);
   - agy: `result.status != SUCCESS` → `status` + `error`/dòng `AGY_ERROR` trong stderr;
   - còn lại: dòng cuối stderr (đã ẩn secret bằng cơ chế redact có sẵn), hoặc trạng thái run.
   Với lỗi đăng nhập (`authentication_failed`, "Failed to authenticate", "Invalid API key", "login") thêm gợi ý: `— đăng nhập lại: CLAUDE_CONFIG_DIR=<dir> claude auth login, hoặc đổi engine triage trong Cài đặt → Hàng đợi`.
4. Telegram: khi intake từ Telegram chuyển `failed`, tin báo lỗi hiện có (tìm nơi gửi thông báo intake failed trong `telegram/`) phải chứa lý do ở Mục tiêu 3. Nếu hiện chưa có thông báo intake failed qua Telegram thì thêm một tin ngắn `Hàng chờ #<id> lỗi: <lý do>` gửi cho chat đã ghép.
5. Thử lại: thêm nút "Thử lại" cho intake `failed` trên màn Điều phối (nhóm Hàng chờ) và API `POST /api/intake/{id}/retry` (chỉ khi `failed` → đặt `queued`, xoá `error`). Alpine CSP: dùng method trả boolean thật cho `:disabled` (xem bài học ở commit 1b72b99).
6. Cài đặt → Hàng đợi: thêm ô chọn "Engine phân tích (triage)" liệt kê `claude` + engine hợp lệ (lấy từ `/api/engines`), lưu cùng nút Lưu.

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/config.rs` | Mục tiêu 1 (trường + kiểm) |
| 2 | `agent-brain/src/orchestrator/triage.rs` | Mục tiêu 2, 3 |
| 3 | `agent-brain/src/orchestrator/mod.rs` | ghi `intake.error` theo Mục tiêu 3; phát sự kiện cho Telegram nếu cần (Mục tiêu 4) |
| 4 | `agent-brain/src/dashboard/api_settings.rs`, `agent-brain/src/dashboard/mod.rs` | Mục tiêu 1 (API, trạng thái sống) |
| 5 | `agent-brain/src/dashboard/api_intake.rs`, `agent-brain/src/db.rs` | Mục tiêu 5 (API retry; hàm DB nếu cần) |
| 6 | `agent-brain/src/telegram/*.rs` (chỉ file gửi thông báo intake) | Mục tiêu 4 |
| 7 | `agent-brain/assets/dashboard/screens/settings.html`, `js/settings.js`, `screens/queue.html`, `js/queue.js`, `app.css` (sinh lại nếu cần) | Mục tiêu 5, 6 |
| 8 | `scripts/dashboard-js-check.mjs` | cập nhật cho khớp |
| 9 | Test: `tests/orchestrator_triage.rs`, `tests/dashboard_settings.rs`, `tests/intake_api.rs`, test Telegram liên quan, unit trong `config.rs`/`triage.rs` | Test |

Không sửa: `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` thật, dự án `srt`, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác.

## Test (bắt buộc, engine giả: `tests/fakes/fake_claude*`, `tests/fakes/fake_agy.py`)
- (a) config: `triage_engine` mặc định `claude`; id engine anthropic/agy hợp lệ; id không tồn tại hoặc engine dsh → lỗi config.
- (b) triage với engine giả anthropic in stream `authentication_failed` → intake `failed`, `error` chứa "Failed to authenticate" và gợi ý đăng nhập lại.
- (c) triage với fake agy trả `result` SUCCESS có JSON `{"tasks":[…]}` → tạo proposal; fake agy treo sau result → vẫn xong (nhờ `finish_after_result`).
- (d) API settings: đổi `triage_engine` sống, GET trả giá trị mới; giá trị sai → 400.
- (e) retry: intake `failed` → `queued`, `error` NULL; intake không `failed` → 409.
- (f) Telegram: intake từ Telegram failed → tin chứa lý do (dùng fake Telegram có sẵn).
- (g) Toàn bộ test cũ pass.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration
bash scripts/build-css.sh --check
bash scripts/dashboard-ui-lint.sh
node scripts/dashboard-js-check.mjs
cd agent-brain && cargo test --no-fail-fast
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"   # không tăng so với base
git diff --stat <base>..HEAD
```
Trình duyệt: serve thử cổng khác, HOME/XDG/`AGENT_BRAIN_DB`/`AGENT_BRAIN_CONFIG` tạm, KHÔNG có `telegram.json`; chỉ dừng PID mình chạy; không đụng serve thật cổng 7437. Kiểm ô chọn engine trong Cài đặt và nút Thử lại; chụp ảnh, ghi đường dẫn tuyệt đối.

## Commit
Rust một hoặc hai commit (`feat(agent-brain): …`), UI một commit (`feat(dashboard): …`). `git add` đúng file, không `--no-verify`, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: file sửa, commit, chỗ lệch plan, kết quả kiểm tra, ảnh chụp.
