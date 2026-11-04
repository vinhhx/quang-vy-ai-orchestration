# Plan: dropdown Bảng công việc liệt kê tất cả dự án

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Bối cảnh (đã kiểm tra trong code)
- `GET /api/board` (`agent-brain/src/dashboard/api_board.rs`) ĐÃ trả `projects` gồm TẤT CẢ dự án, mỗi dự án có `has_board` (true khi có `.planning/ROADMAP.md`). Backend không cần đổi.
- Frontend lọc lại trong `agent-brain/assets/dashboard/js/board.js`, getter `boardProjects` (khoảng dòng 701–714): chỉ đưa vào dropdown dự án có `p.has_board` (hoặc dự án đang chọn). Đây là nguyên nhân dropdown chỉ còn 1 dự án (QuangVy-Orchestration) trong 180 dự án.
- Khi chọn dự án không có bảng, backend trả `state: "empty"`, `reason: "no_roadmap"`, kèm `project` và `projects`. Frontend `apply()` (khoảng dòng 285–305) đã xử lý trường hợp này và hiện thông báo "Chưa có bảng công việc" (`data-board-empty`).
- Khu vực "Công việc đã giao" (`board.html`, `<section data-board-tasks>`) nằm ngoài các `x-show` trạng thái, nên vẫn hiện khi dự án không có bảng GSD. Nó lọc task theo `projectSel`.
- Test `board02_default_project_and_projects_list` và `board02_no_board_projects` (`agent-brain/tests/dashboard_board.rs`) khóa hợp đồng backend: giữ nguyên, KHÔNG sửa.

## Quyết định
- Chỉ đổi frontend: bỏ bộ lọc `has_board` trong dropdown. Dự án không có bảng hiện trong dropdown với nhãn phụ.
- Mặc định vẫn là dự án có bảng đầu tiên (backend không đổi).

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/assets/dashboard/js/board.js` | Getter `boardProjects`: đưa VÀO MỌI dự án trong `this.projects` (bỏ điều kiện `p.has_board`). Giữ logic thêm dự án đang chọn nếu chưa có trong danh sách (`curId && !seen`). Giữ nguyên thứ tự của `this.projects`. |
| 2 | `agent-brain/assets/dashboard/js/board.js` | Hàm `projectLabel(p)` (khoảng dòng 497): nếu `p.has_board === false` thì trả `(p.name_vi || p.id) + ' · chưa có bảng GSD'`; nếu không thì giữ nguyên `p.name_vi || p.id`. Dự án đang chọn không có `has_board` trong danh sách (trường hợp thêm từ `this.project`) cũng hiện nhãn này. |
| 3 | `agent-brain/assets/dashboard/js/board.js` | Không đổi các hàm khác. Không đổi `apply`, `load`, `projectChanged`. |

KHÔNG sửa: `agent-brain/src/**`, `agent-brain/tests/**`, `agent-brain/assets/dashboard/screens/board.html` (trừ khi cần, và nếu có thì ghi rõ trong báo cáo), `agent-brain/README.md`.

## Không được đụng tới
- Các file có thay đổi chưa commit của người dùng: `agent-brain/assets/dashboard/js/settings.js`, `agent-brain/assets/dashboard/screens/settings.html`, `agent-brain/assets/prompts/lead.md`, `agent-brain/src/config.rs`, `agent-brain/src/dashboard/api_settings.rs`, `agent-brain/src/engine.rs`, `agent-brain/src/skills.rs`, `agent-brain/tests/dashboard_settings.rs`. Không sửa, không stage, không commit.
- `.planning/STATE.md`, `.planning/ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`, `~/.claude-orca`, `~/.config/agent-brain/*`.
- `git add -A`, `--no-verify`: cấm. Stage theo đường dẫn.

## Kiểm tra (chạy trong `agent-brain/`)
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --test dashboard_board --test dashboard_assets --test dashboard_http 2>&1 | grep -E "^test result|FAILED|panicked"
git -C .. diff --stat -- agent-brain/assets/dashboard/js/board.js
```
Kiểm tra logic (không cần trình duyệt): đọc lại getter `boardProjects` và `projectLabel`, xác nhận: (a) mọi dự án trong `projects` đều vào dropdown; (b) dự án không có bảng có nhãn "chưa có bảng GSD"; (c) thứ tự không đổi.

## Tiêu chí nghiệm thu
1. Dropdown "Bảng" liệt kê tất cả dự án trong `projects` (180 dự án với cấu hình hiện tại).
2. Dự án không có `ROADMAP.md` có nhãn "· chưa có bảng GSD"; dự án có bảng không có nhãn đó.
3. Chọn dự án không có bảng: trang hiện thông báo "Chưa có bảng công việc" và khu vực "Công việc đã giao" vẫn hiện task của dự án đó.
4. Mọi test cũ pass (không đổi backend). Chỉ `board.js` bị sửa.

## Quy trình commit
- Một commit: `feat(agent-brain): board dropdown lists all projects`, kèm dòng `Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>`.
- Không push.

## Báo cáo cuối (agy phải ghi)
File đã sửa, hash commit, kết quả nguyên văn các lệnh verify, chỗ lệch plan (nếu có), phần chưa kiểm tra (giao diện thật trên trình duyệt).
