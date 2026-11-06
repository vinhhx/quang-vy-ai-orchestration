# Plan: bật/tắt hàng đợi (Orchestrator) và chỉnh thông số ngay trong màn Cài đặt, có hiệu lực không cần khởi động lại serve

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.
Làm SAU khi hai nhánh đang chạy đã merge vào `main` (`queue-action-buttons-csp-fix` sửa `queue.html`/`queue.js`; `dev-continue-after-failed-run` sửa `dashboard/api_tasks.rs`, `dashboard/taskstate.rs`). Rebase lên `main` mới nhất trước khi bắt đầu.

## Hiện trạng (đã đọc code)
- `config.rs:31` `OrchestratorConfig { enabled, max_parallel (1–10), auto_resume_quota, resume_after_minutes (1–1440) }`, mặc định `enabled: false`. Đọc một lần khi serve khởi động vào `AppState.cfg` (`dashboard/mod.rs:33`, bản sao tĩnh).
- Nơi đọc: `orchestrator/mod.rs:46` (`tick`), `orchestrator/scheduler.rs:42,47,199–200`, `dashboard/api_tasks.rs:~828` (`create_task`: bật thì vào hàng chờ), `dashboard/api_intake.rs:~470–489, ~549–550` (trạng thái/board), `telegram/intake.rs:~670, ~1244` (Telegram `/task`: bật thì vào hàng chờ).
- Vòng điều phối luôn được spawn (`dashboard/mod.rs:246` `orchestrator::spawn_loop`) và tự kiểm `enabled` mỗi nhịp → đổi giá trị lúc chạy là đủ, không cần spawn lại.
- Ghi config.json đã có mẫu: `dashboard/api_settings.rs::save_engine_models_blocking` dùng `settings_store::read_doc` + `write_object(path, version, |root| …)` (có kiểm xung đột phiên bản).
- Người dùng muốn: bật hàng đợi (đã bật tay trong `~/.config/agent-brain/config.json`) và có công tắc bật/tắt trong Cài đặt.

## Mục tiêu
1. Thông số orchestrator là trạng thái SỐNG trong `AppState`: thêm `orch: std::sync::RwLock<crate::config::OrchestratorConfig>` (khởi tạo từ `cfg.orchestrator`) và accessor `pub fn orch(&self) -> OrchestratorConfig` (trả bản sao). Thay MỌI chỗ đọc `st.cfg.orchestrator.*` / `ctx.st.cfg.orchestrator.*` ở danh sách trên bằng `st.orch().*`. `cfg.orchestrator` vẫn còn (giá trị lúc khởi động) nhưng không được đọc ở đường chạy nữa (grep để chắc).
2. API mới trong `dashboard/api_settings.rs`:
   - `GET /api/orchestrator/settings` → `{ enabled, max_parallel, auto_resume_quota, resume_after_minutes, version }` (version của config.json như các API settings khác).
   - `POST /api/orchestrator/settings` body `{ enabled, max_parallel, auto_resume_quota, resume_after_minutes, version }`: kiểm hợp lệ đúng luật của `config.rs` (boolean; 1–10; 1–1440; lỗi trả theo định dạng `FieldError` hiện có), ghi khối `"orchestrator"` vào config.json bằng `settings_store::write_object` (giữ nguyên các khoá khác; xung đột phiên bản → trả lỗi xung đột như API hiện có), rồi cập nhật `AppState.orch`. Trả giá trị mới + version mới.
   - Cùng cơ chế xác thực token/CSRF như các route settings khác (đặt trong router settings hiện có).
3. Khi TẮT: việc đang chạy và đề xuất đã duyệt vẫn chạy tiếp như cũ; intake mới không vào hàng chờ (đi thẳng như khi tắt hiện nay); intake đang `queued` giữ nguyên, sẽ được xử lý khi bật lại. Ghi đúng hành vi này ở dòng mô tả trong UI.
4. Màn Cài đặt (`screens/settings.html`, `js/settings.js`): thêm mục "Hàng đợi (Orchestrator)" ĐẦU TIÊN, trước "Đội hình":
   - Công tắc bật/tắt (`role="switch"`, `aria-checked`), lưu ngay khi bấm (có trạng thái đang lưu, lỗi hiện bên dưới; xung đột phiên bản → thông báo "Config vừa đổi ở nơi khác — tải lại" và nạp lại).
   - Ô số "Số việc chạy song song" (1–10), công tắc "Tự chạy lại sau khi hết quota", ô số "Chạy lại sau (phút)" (1–1440, chỉ bật khi công tắc trên bật); nút "Lưu" cho 3 ô này.
   - Một dòng mô tả: "Bật: việc giao từ dashboard/Telegram vào hàng chờ, Lead phân tích thành đề xuất để bạn duyệt. Tắt: việc giao chạy thẳng. Có hiệu lực ngay, không cần khởi động lại serve."
   - Alpine CSP (logic trong `settings.js`, không biểu thức phức tạp trong thuộc tính, không truy cập theo khoá động `obj[expr]`), focus-visible như các nút khác, hỗ trợ sáng/tối.
5. Màn Điều phối (`screens/queue.html`): cảnh báo "Orchestrator đang tắt…" đổi phần hướng dẫn thành link "Bật trong Cài đặt" (`href="#/settings"`), bỏ đoạn hướng dẫn sửa config.json và khởi động lại. Badge "Orchestrator: Bật/Tắt" và slot dùng dữ liệu sống (API board đã đọc `st.orch()` sau Mục tiêu 1, nên chỉ cần polling hiện có).
6. Telegram: không thêm lệnh mới; chỉ đọc giá trị sống (Mục tiêu 1).

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/dashboard/mod.rs` | `AppState.orch` + accessor, khởi tạo |
| 2 | `agent-brain/src/orchestrator/mod.rs`, `agent-brain/src/orchestrator/scheduler.rs` | đọc `st.orch()` |
| 3 | `agent-brain/src/dashboard/api_tasks.rs`, `agent-brain/src/dashboard/api_intake.rs` | đọc `st.orch()` |
| 4 | `agent-brain/src/telegram/intake.rs` | đọc `ctx.st.orch()` |
| 5 | `agent-brain/src/dashboard/api_settings.rs` | API GET/POST (Mục tiêu 2) |
| 6 | `agent-brain/assets/dashboard/screens/settings.html`, `agent-brain/assets/dashboard/js/settings.js` | Mục tiêu 4 |
| 7 | `agent-brain/assets/dashboard/screens/queue.html` | Mục tiêu 5 (chỉ khối cảnh báo) |
| 8 | `agent-brain/assets/dashboard/app.css` | Sinh lại bằng `bash scripts/build-css.sh` nếu lớp đổi |
| 9 | Test: `agent-brain/tests/dashboard_settings.rs` (API), test có sẵn liên quan orchestrator (`tests/orchestrator_scheduler.rs`, `tests/intake_api.rs`, `tests/telegram_intake.rs`) chỉ khi cần cập nhật cách dựng `AppState` | Test |
| 10 | `scripts/dashboard-js-check.mjs` | Chỉ khi cần cho khớp markup mới |

Không sửa: `~/.config/agent-brain/*` thật, `~/.local/share/agent-brain/*`, dự án `srt`, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác.

## Test (bắt buộc)
- (a) API: GET trả giá trị đang dùng + version; POST hợp lệ ghi đúng khối `orchestrator` trong config.json tạm (các khoá khác giữ nguyên byte-cho-byte về nội dung JSON), cập nhật trạng thái sống (GET ngay sau đó trả giá trị mới, `/api/orchestrator/status` cũng vậy); POST sai kiểu/ngoài khoảng → 400 với lỗi theo trường; version cũ → lỗi xung đột, file không đổi.
- (b) Sống: tắt qua API → `create_task` chạy thẳng (không tạo intake); bật qua API → `create_task` tạo intake; `orchestrator::tick` không xử lý intake khi tắt, xử lý khi bật (dùng AppState giả như test hiện có).
- (c) Toàn bộ test cũ pass.

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
Trình duyệt (serve thử cổng khác, HOME/XDG/`AGENT_BRAIN_DB`/`AGENT_BRAIN_CONFIG` tạm, KHÔNG có `telegram.json` để không tranh bot với serve thật, chỉ dừng PID mình khởi chạy; không đụng serve thật cổng 7437): bật/tắt công tắc → badge trên Điều phối đổi trong một nhịp polling mà không khởi động lại; config.json tạm có khối `orchestrator` đúng; lỗi nhập ngoài khoảng hiện đúng chỗ. Chụp ảnh mục mới (sáng/tối), ghi đường dẫn tuyệt đối.

## Commit
Rust (Mục tiêu 1–3, 6) một commit `feat(agent-brain): orchestrator settings are live and editable over the API`; UI (Mục tiêu 4–5) một commit `feat(dashboard): Cài đặt toggles the orchestrator queue`. `git add` đúng file, không `--no-verify`, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: file sửa, commit, chỗ lệch plan, kết quả kiểm tra, ảnh chụp.
