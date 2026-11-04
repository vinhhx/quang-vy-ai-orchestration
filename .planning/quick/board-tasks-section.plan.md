# Plan: thêm khu vực "Công việc đã giao" vào Bảng công việc (/board)

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Bối cảnh (đã kiểm tra trong code)
- `/board` hiện chỉ đọc lộ trình GSD (`.planning/ROADMAP.md`, plan) qua `GET /api/board`. `agent-brain/src/dashboard/api_board.rs` ghi rõ: không mở `brain.db`. Các task tạo từ "Giao việc" (và Telegram `/task`) nằm ở bảng `tasks` của `brain.db`, và chỉ hiện ở màn Task / Tổng quan.
- `GET /api/tasks?limit=100[&profile=…]` (`agent-brain/src/dashboard/api_tasks.rs`, hàm `list_tasks`, khoảng dòng 850–1000) đã trả mỗi task với: `id`, `project` (slug dự án), `project_name`, `profile`, `title`, `slug`, `branch`, `status`, `banner`, `group`, `current_step`, `steps`, `auto_state`, `stop_reason`, `human_mode`, `human_name`, `quota_detail`, `created_at`, `updated_at`. Trường `group` đã được tính sẵn bởi `agent-brain/src/dashboard/taskstate.rs` (`derive_with`).
- Tổng quan đã dùng endpoint này: `agent-brain/assets/dashboard/js/overview.js` (dòng ~34, `window.AB.profileQuery()`).
- Bảng công việc GSD: `agent-brain/assets/dashboard/js/board.js` (709 dòng) và `agent-brain/assets/dashboard/screens/board.html` (167 dòng). Trang dùng Alpine (`x-data="board"`), CSP, Tailwind; class phải viết đủ chuỗi literal trong file JS để Tailwind quét được (xem `BOARD_CARD` trong `board.js`).

## Quyết định (đã chọn, lý do: không đổi API backend, không mở rộng quyền đọc)
- **Chỉ đổi frontend.** Khu vực mới lấy dữ liệu từ `GET /api/tasks` có sẵn, KHÔNG thêm route, KHÔNG sửa `api_board.rs`, KHÔNG sửa `api_tasks.rs`, KHÔNG sửa `taskstate.rs`.
- Khu vực nằm **dưới** bảng GSD hiện có, tiêu đề "Công việc đã giao", chỉ hiện task của dự án đang chọn trên bảng.
- Nhóm theo `group` đã có (giá trị lấy từ `taskstate.rs`, không tự đặt thêm nhóm).

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/assets/dashboard/js/board.js` | Thêm state `tasks: []`, `tasksLoaded: false`, `tasksError: ''`. Thêm hàm `loadTasks()` gọi `window.AB.api('GET', '/api/tasks?limit=100' + profile)` (ghép profile như `overview.js`), gán `res.data.tasks`. Gọi `loadTasks()` cùng lúc với tải bảng GSD và khi đổi dự án. Thêm computed `boardTasks`: lọc `tasks` theo dự án đang chọn (so `task.project` với id dự án của bảng). Thêm computed `taskGroups`: nhóm `boardTasks` theo `group` theo đúng thứ tự hiển thị (xem bước 3). |
| 2 | `agent-brain/assets/dashboard/screens/board.html` | Thêm một `<section>` bên dưới phần bảng GSD, tiêu đề `Công việc đã giao`. Mỗi nhóm có tiêu đề và danh sách thẻ: tiêu đề task (`title`), `project_name`, `banner`, `current_step`. Khi không có task: dòng "Chưa có công việc nào được giao cho dự án này." Khi lỗi: dòng lỗi và nút tải lại. |
| 3 | (trong `board.js`, `taskGroups`) | Thứ tự và nhãn nhóm: `running` → "Đang xử lý"; `waiting` → "Chờ bạn"; `attention` → "Cần xử lý"; `done` → "Đã xong". Nhóm không có task thì ẩn. Nếu `group` có giá trị khác bốn giá trị này (agy phải kiểm tra trong `taskstate.rs` bằng `grep -n 'group' src/dashboard/taskstate.rs`), đặt vào nhóm "Khác" ở cuối, không bỏ. |
| 4 | Test | Xem mục "Kiểm tra". Thêm test trong `agent-brain/tests/dashboard_assets.rs` (đã có, kiểm tra asset nhúng) nếu file đó có sẵn cách kiểm tra chuỗi trong asset; nếu không, KHÔNG thêm test mới, ghi rõ trong báo cáo là không có test tự động cho phần này. |

## Quy tắc khi làm
- Đọc `overview.js` và `board.js` trước; bám đúng kiểu code đang có (Alpine, `window.AB.api`, `window.AB.errorText`, `window.AB.profileQuery`).
- Id dự án của bảng GSD và slug trong `task.project`: kiểm tra trong `agent-brain/src/projects.rs` và `agent-brain/src/dashboard/api_board.rs` (`project_summary` dùng `p.id`) xem hai giá trị có trùng nhau không. Nếu KHÔNG trùng, dừng và báo cáo, không tự đoán cách ghép.
- Class Tailwind phải là chuỗi literal đầy đủ trong `board.js` hoặc `board.html`. Không ghép chuỗi động kiểu `'bg-' + color`.
- Hỗ trợ sáng/tối (`dark:`) như phần còn lại của bảng. Chiều rộng điện thoại: không cuộn ngang trang.
- Không dùng màu hoặc icon mới ngoài bộ đang có trong `board.html`/`board.js`.

## Không được đụng tới
- `agent-brain/src/**` (cả backend), `agent-brain/Cargo.*`, `agent-brain/README.md`.
- Các file đang có thay đổi chưa commit của người dùng: `agent-brain/assets/dashboard/js/settings.js`, `agent-brain/assets/dashboard/screens/settings.html`, `agent-brain/assets/prompts/lead.md`, `agent-brain/src/config.rs`, `agent-brain/src/dashboard/api_settings.rs`, `agent-brain/src/engine.rs`, `agent-brain/src/skills.rs`, `agent-brain/tests/dashboard_settings.rs`. Không sửa, không stage, không commit các file này.
- `.planning/STATE.md`, `.planning/ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`, `~/.claude-orca`.
- `git add -A`, `--no-verify`: cấm. Stage theo đường dẫn.

## Kiểm tra (chạy trong `agent-brain/`)
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --test dashboard_board --test dashboard_assets --test dashboard_http 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"
git -C .. diff --stat -- agent-brain/assets/dashboard/js/board.js agent-brain/assets/dashboard/screens/board.html
```
Kiểm tra thủ công (nếu chạy được app): `agent-brain serve`, mở `/board`, xác nhận khu vực "Công việc đã giao" hiện đúng các task của dự án đang chọn, và đổi dự án thì danh sách đổi theo. Nếu không chạy được giao diện, ghi rõ là chưa kiểm tra giao diện.

## Tiêu chí nghiệm thu
1. `/board` có khu vực "Công việc đã giao" dưới bảng GSD; task đang chạy nằm nhóm "Đang xử lý", task đã xong nằm nhóm "Đã xong".
2. Chỉ hiện task của dự án đang chọn; đổi dự án thì danh sách đổi.
3. Không có task: hiện dòng trống; lỗi `/api/tasks`: hiện thông báo lỗi, bảng GSD vẫn hiển thị.
4. Bảng GSD (xong / sẵn sàng / bị chặn, quan hệ, milestone) không đổi hành vi.
5. Mọi test cũ vẫn pass. Chỉ `board.js` và `board.html` (cộng test nếu có) bị sửa.

## Quy trình commit
- Một commit: `feat(agent-brain): tasks section on the board`, kèm dòng `Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>`.
- Không push.

## Báo cáo cuối (agy phải ghi)
File đã sửa, hash commit, các lệnh verify và kết quả nguyên văn, chỗ lệch plan (nếu có), phần chưa kiểm tra (giao diện thật, test tự động nếu không có).
