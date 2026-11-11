# Plan: quét dự án ra danh sách ứng viên, ẩn dự án, tìm/lọc trên bảng

Repo: /mnt/data/IDE/QuangVy-Orchestration (nhánh main), crate `agent-brain/`. Làm trong worktree của bạn.
**Điều kiện bắt đầu:** `main` đã chứa kết quả của các plan `projects-table-view` và `projects-ui-alias-workspace` (màn Dự án dạng bảng,
cột Nhánh gốc, Workspace trong Nâng cao). Nếu `git log` chưa có commit `feat(dashboard): projects table shows base branch...` thì dừng và báo lại.

## Vấn đề
`projects::scan` (`src/projects.rs:1144`) thêm MỌI thư mục con có `.git` trực tiếp dưới mỗi `scan_roots` thành dự án, không lọc, không hỏi.
Thực tế `projects_map.json` có 182 dự án (125 từ thư mục laravel, 52 từ yii2), chỉ 12 dự án có task. Không có cách ẩn/xóa một dự án;
quét lại luôn thêm lại thứ vừa dọn. Bảng Dự án 182 dòng khó tìm.

## Yêu cầu (người dùng đã duyệt)
1. **Quét ra danh sách ứng viên, tick chọn rồi mới thêm.** Nút "Quét lại" trên dashboard không tự thêm dự án nữa.
2. **Ẩn dự án** (cần để người dùng ẩn hàng loạt dự án không có task sau khi tính năng xong; việc ẩn hàng loạt tôi làm riêng, KHÔNG nằm trong plan này).
3. **Tìm và lọc trên bảng Dự án.**

## Thiết kế

### A. Quét ra ứng viên (backend `src/projects.rs`, `src/dashboard/api_projects.rs`)
- Tách `scan` thành hai phần dùng chung helper:
  - `refresh_known(data)`: phần cập nhật dự án ĐÃ CÓ (đặt/bỏ `missing`, cập nhật `repo`), giữ đúng hành vi cũ.
  - `scan_candidates(data) -> Vec<ScanCandidate>`: liệt kê repo (có `.git`, thư mục con trực tiếp của `scan_roots` hợp lệ) CHƯA có trong `projects` (so theo realpath, như `projects_by_realpath`);
    mỗi ứng viên: `path`, `id` đề xuất (dùng `new_entry_from_projects`/quy tắc đặt id hiện có, kể cả hậu tố khi trùng), `name`, `repo` (remote), `stack` (`detect_stack`), `last_commit_at` (ngày commit gần nhất, rẻ: đọc `git log -1 --format=%ct`
    với timeout ngắn hoặc dùng mtime của `.git/HEAD`/refs; ưu tiên cách không spawn git nếu được, giống phong cách của `git_remote`; lỗi thì để trống). Không ghi file.
  - Dự án có `"hidden": true` vẫn nằm trong `projects` nên KHÔNG hiện làm ứng viên.
- CLI `agent-brain projects scan` (`src/main.rs`): GIỮ nguyên hành vi thêm tất cả (script/CLI đang dựa vào) nhưng thêm cờ `--list` in ứng viên mà không ghi, và kiểm tra test CLI hiện có vẫn pass.
- API:
  - `POST /api/projects/scan` giữ đường dẫn nhưng đổi hành vi: chạy `refresh_known` + trả `{candidates: [...], bad_roots: [...], refreshed: {missing, restored}}`; KHÔNG thêm dự án.
    (Đừng gọi tiến trình con `projects scan` nữa; gọi hàm thẳng, ghi map bằng đường ghi nguyên tử đang dùng ở các handler khác với `etag`.)
  - `POST /api/projects/scan/add` body `{paths: [string]}`: thêm đúng các đường dẫn đó (mỗi đường dẫn phải là ứng viên hợp lệ nằm dưới một `scan_root`, có `.git`, chưa có trong map; từ chối đường dẫn lạ bằng 422, không bao giờ thêm đường dẫn ngoài scan_roots), trả danh sách dự án đã thêm; gọi `db::sync_projects`.
  - Giữ nguyên `GET /api/projects/suggest`, `POST /api/projects` (thêm tay).
- Test: ứng viên không chứa dự án đã có hoặc đã ẩn; `scan/add` từ chối path ngoài root, path trùng; `refresh_known` giữ cờ `missing`; không có thay đổi map khi chỉ gọi quét.

### B. Ẩn dự án
- Khóa `hidden: bool` trong mục dự án của `projects_map.json` (vắng = false). `Project` có thêm `hidden: bool`; `Project::from_value` đọc khóa.
- API: `PUT /api/projects/{id}/hidden` body `{hidden: bool, etag}` và `PUT /api/projects/hidden` body `{ids: [string], hidden: bool, etag}` (hàng loạt, một lần ghi, một bản sao lưu map nếu code hiện đã sao lưu ở các thao tác tương tự; nếu chưa, đừng thêm cơ chế sao lưu mới). Không xóa gì khỏi map.
- `projects_of` vẫn trả TẤT CẢ (task cũ, lịch sử, thống kê cần phân giải dự án đã ẩn). Thêm `visible_projects_of` (hoặc `Project::is_visible`) và dùng ở các nơi CHỌN dự án cho việc MỚI: màn Giao việc (`assign`), intake, gợi ý/triage của orchestrator (`src/orchestrator/*`), danh sách chọn dự án trong dashboard, Telegram chọn dự án.
  Đọc từng nơi gọi `projects_of(` (grep) và quyết định đúng: chỗ chọn dự án cho việc mới thì lọc; chỗ phân giải dự án của task/run đã có thì KHÔNG lọc. Ghi quyết định từng chỗ vào báo cáo.
- Dự án đã ẩn: task đang có vẫn chạy bình thường; chỉ không được chọn cho task mới. `GET /api/projects` trả cả dự án ẩn kèm `hidden` và `task_count` (số task của dự án, lấy từ `brain.db`; nếu đã có trường tương đương thì dùng lại).

### C. Giao diện bảng Dự án (`assets/dashboard/screens/projects.html`, `js/projects.js`, `app.css` dựng lại)
- Thanh công cụ phía trên bảng: ô tìm (khớp không phân biệt hoa thường với id, tên, alias, đường dẫn), nút lọc: "Có task", "Telegram bật", "Chưa có stack", "Hiện cả dự án đã ẩn" (mặc định ẩn các dự án `hidden`). Hiển thị "N / tổng dự án". Trạng thái lọc chỉ ở trình duyệt (có thể nhớ bằng localStorage trong try/catch).
- Mỗi dòng: nút "Ẩn"/"Bỏ ẩn" (aria-label có tên dự án), badge "Đã ẩn" khi hiện dự án ẩn, và số task. Chọn nhiều dòng bằng checkbox rồi "Ẩn mục đã chọn" (gọi API hàng loạt).
- "Quét lại": gọi `POST /api/projects/scan`, hiện **bảng ứng viên** (checkbox từng dòng, "Chọn tất cả", bộ đếm, ô lọc nhanh, cột Đường dẫn/Stack/Remote/Commit gần nhất) và nút "Thêm mục đã chọn" gọi `scan/add`. Không chọn gì thì không thêm gì. Giữ thông báo bad_roots. Không còn dòng stdout thô như trước (xóa `scanOut` nếu không còn dùng, cập nhật test).
- Giữ a11y, dark mode, cuộn ngang trên màn hẹp, mọi id/handler hiện có mà test đang dùng. Danh sách 182+ dòng: không cần phân trang nhưng đừng gây giật (tính danh sách lọc một lần bằng getter).
Không đụng: `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` chưa tracked, `~/.claude-orca`, `screens/task.html`, `js/task.js`.

## Các bước
1. Đọc `src/projects.rs` (scan, new_entry_from_projects, git_remote, load/save map), `src/dashboard/api_projects.rs` (scan_projects, list_projects, handler PUT mẫu có etag, add_project_api), `assets/dashboard/screens/projects.html` + `js/projects.js` (sau các plan trước), `tests/dashboard_projects.rs`, và mọi chỗ gọi `projects_of(`.
2. Backend A + test; commit `feat(agent-brain): scan lists candidates instead of adding every repo`.
3. Backend B + test; commit `feat(agent-brain): hide projects without removing them`.
4. Frontend C; kiểm tra bằng trình duyệt headless trên instance `agent-brain serve` thử (cổng khác, dữ liệu sandbox có ~30 repo giả dưới một scan_root, KHÔNG đụng cổng 7437 hay `~/.config/agent-brain` thật: đặt biến môi trường/đường dẫn cấu hình sandbox như các test dashboard đang làm; dừng instance và Chrome sau khi xong).
   Kiểm: quét liệt kê ứng viên mà không thêm; tick 2 rồi thêm → chỉ 2 xuất hiện; quét lại không liệt kê lại; ẩn/bỏ ẩn; tìm/lọc; 480px không tràn. Commit `feat(dashboard): scan candidates table, hide and filter on the projects screen`.
5. Không `git add -A`, không `--no-verify`; mỗi commit kết thúc bằng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Tiêu chí nghiệm thu
- Nhấn "Quét lại" không thay đổi `projects_map.json` ngoài `missing`/`repo` của dự án đã có; thêm chỉ khi người dùng tick và xác nhận.
- Dự án đã ẩn không bao giờ hiện làm ứng viên và không xuất hiện ở nơi chọn dự án cho việc mới, nhưng task/run/thống kê cũ của nó vẫn hiển thị đúng.
- API từ chối thêm đường dẫn ngoài `scan_roots` (422).
- Bảng có ô tìm và lọc hoạt động trên 182+ dòng; mặc định ẩn dự án đã ẩn.
- CLI `agent-brain projects scan` vẫn thêm tất cả như cũ; `--list` chỉ liệt kê.

## Lệnh kiểm tra (trong `agent-brain/`, từ worktree)
- `cargo test` toàn bộ nếu thời gian cho phép; tối thiểu `--lib` và mọi test file nhắc tới `projects`/`scan` (`grep -l`), cùng `dashboard_projects`, `dashboard_assets`, `dashboard_http`.
- `cargo build --release`
- `bash spikes/lib/scan-secrets.sh` ở root repo

## Báo cáo cuối
File đổi, mã commit, nhánh/worktree, kết quả test và kiểm tra trình duyệt, quyết định lọc từng nơi gọi `projects_of`, điểm lệch plan.
Không merge, không push, không dừng/khởi động lại `serve`, không thay `~/.local/bin/agent-brain`.
