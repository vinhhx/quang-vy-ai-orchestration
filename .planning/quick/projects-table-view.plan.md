# Plan: màn "Dự án" chuyển sang dạng bảng, sửa ngay trên dòng

Repo: /mnt/data/IDE/QuangVy-Orchestration (nhánh main). Chỉ frontend, **không đổi API/backend**.

## Vấn đề
`agent-brain/assets/dashboard/screens/projects.html` (421 dòng) hiện hiển thị mỗi dự án là một thẻ cao
(~15 khối field xếp dọc: profile, alias có nút "Sửa" mới mở ô nhập, Telegram, stack, nguồn đọc, timeouts,
protocol dir, base branch, workspace, pins, preflight...). Khó quét, khó so sánh, phải bấm "Sửa" rồi mới nhập.
Mong muốn: giống bảng thành viên ở `screens/departments.html` (dòng 108-185, `<table>` + `<thead>` + `<tbody>`),
sửa trực tiếp trên dòng, không phải bấm để mở form chi tiết cho những field hay dùng.

## Phạm vi file (chỉ các file này)
- `agent-brain/assets/dashboard/screens/projects.html` (thay khối `x-for="p in projects"` ~ dòng 150-340)
- `agent-brain/assets/dashboard/js/projects.js` (chỉ thêm state/hàm cho việc mở dòng nâng cao; KHÔNG đổi
  hàm gọi API, tên hàm, id phần tử đang dùng)
- `agent-brain/assets/dashboard/app.css` chỉ khi cần (nếu dùng Tailwind build thì xem `dashboard-src/input.css`;
  ưu tiên dùng class Tailwind đã có trong departments.html để khỏi phải build lại CSS)
Không đụng: backend `src/`, `tests/` (trừ khi bước 5), `.planning/STATE.md`, `ROADMAP.md`, `~/.claude-orca`.

## Thiết kế
Bảng bọc trong `<div class="overflow-x-auto">`, mẫu class lấy từ departments.html. Mỗi dự án một `<tr>`:

| Cột | Nội dung | Sửa |
|---|---|---|
| Dự án | `name_vi` (đậm), `id` (mono nhỏ), badge "thiếu thư mục" nếu `p.missing` | không |
| Đường dẫn / loại | `p.path` (mono, `break-all`), `p.type` | không |
| Profile | `<select>` hiện có (giữ nguyên handler + id từ `profileSelectId(p)`) | ngay trên dòng |
| Alias | chip alias + ô input nhỏ; Enter hoặc blur = lưu (dùng lại hàm lưu alias hiện có, bỏ nút "Sửa"/"Hủy" tách bước) | ngay trên dòng |
| Telegram | checkbox hiện có (giữ `telegramLocked(p)`) | ngay trên dòng |
| Workspace | `<select>` shared/worktree + số task song song (giữ handler hiện có) | ngay trên dòng |
| Trạng thái | nút "Kiểm tra" (status zero-token hiện có), kết quả hiện ở dòng mở rộng | nút |
| Thao tác | nút "Nâng cao" bật/tắt dòng mở rộng (`aria-expanded`) | nút |

Dòng mở rộng (`<tr x-show="isExpanded(p)"><td colspan="8">`) chứa các field ít dùng, **giữ nguyên markup, id,
handler của chúng**: stack công nghệ, dự án nguồn (chỉ đọc), timeouts theo vai, thư mục file giao thức,
nhánh gốc + 2 checkbox git, ghim thành viên/engine, preflight, và khối kết quả status.
Thêm vào `projects.js`: `expanded: {}`, `isExpanded(p)`, `toggleExpanded(p)`.

Yêu cầu giữ: a11y (`<th scope="col">`, `aria-label` có tên dự án trên nút, `aria-expanded`), dark mode,
`min-h-12`/`min-h-9` cho vùng bấm, thông báo lỗi `x-show="...Error(p)"` vẫn hiện (đặt trong ô tương ứng
hoặc dòng mở rộng, nhưng lỗi của field nằm ở dòng mở rộng thì tự mở dòng đó: `expanded[p.id] = true` khi có lỗi).
Giữ nguyên mọi phần phía trên danh sách (thư mục gốc quét, thêm dự án, cảnh báo) và phần "Quy trình" bên dưới.

## Các bước
1. Đọc đủ `projects.html`, `projects.js`, `departments.html` (bảng members) và `tests/dashboard_projects.rs`,
   `tests/dashboard_assets.rs`.
2. Thay khối thẻ bằng bảng + dòng mở rộng như trên trong `projects.html`.
3. Thêm `expanded`/`isExpanded`/`toggleExpanded` vào `projects.js`; alias lưu khi Enter/blur bằng hàm sẵn có.
4. Dự án rỗng/đang tải/lỗi: giữ nguyên các khối `isEmpty`, `isLoading`, `hasError`.
5. Nếu test nào bám vào markup cũ (selector/id đã bỏ) thì sửa test cho khớp markup mới, nhưng không làm
   yếu assertion. Không đổi id phần tử mà test/JS đang dùng nếu tránh được.
6. Commit từng nhiệm vụ: `feat(dashboard): projects screen as table with inline edit`. Không `git add -A`,
   không `--no-verify`.

## Tiêu chí nghiệm thu
- Màn Dự án hiện một bảng, mỗi dự án một dòng; thấy profile, alias, Telegram, workspace mà không phải bấm gì.
- Đổi profile, sửa alias (Enter/blur), bật/tắt Telegram, đổi workspace hoạt động như trước (cùng API, cùng lỗi 409/422).
- "Nâng cao" mở dòng chứa đủ các field cũ, lưu được như trước; không field nào bị mất.
- Cuộn ngang được trên màn hẹp, không vỡ layout; dark mode đúng.
- Không còn khối thẻ cao theo dự án.

## Verify
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --test dashboard_assets --test dashboard_projects --test project_read_sources 2>&1 | tail -30
git diff --stat HEAD~1   # chỉ các file trong phạm vi
grep -n "<table" assets/dashboard/screens/projects.html   # có bảng
```
Nếu chạy được dashboard (`agent-brain serve`) thì mở màn Dự án kiểm tay các thao tác trên; nếu không, ghi rõ trong summary là chưa kiểm tay.

## Summary agy ghi lại
File đã đổi, commit, deviation, kết quả từng lệnh verify, phần nào chưa kiểm tay.
