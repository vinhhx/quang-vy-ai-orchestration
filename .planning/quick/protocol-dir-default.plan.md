# Plan: file giao thức (plan / QA / review) không còn nằm ở root dự án

Repo: /mnt/data/IDE/QuangVy-Orchestration (nhánh main), crate `agent-brain/`. Chỉ backend Rust. Làm trong worktree của bạn.

## Vấn đề
Dự án không đặt `protocol_dir` trong `projects_map.json` thì 4 file giao thức nằm ở root repo dự án:
`.plan.md`, `.qa-report.md`, `.plan-review.md`, `.code-review.md` (`src/protocol_paths.rs`, hằng `PLAN_FILE`...).
Muốn: mặc định mới, theo từng task, nằm trong thư mục tri thức của dự án.

## Hành vi mới (đã được người dùng duyệt)
Khi mục dự án trong `projects_map.json` **không có khóa `protocol_dir`**, `protocol_dir` hiệu lực là:
- dự án có thư mục thật `.planning/` (không phải symlink) → `.planning/quick/plan-{slug}`
- ngược lại (không có `.planning/`, có hay không có `.gsd/`) → `.gsd/quick/plan-{slug}`

Mỗi task có thư mục riêng, ví dụ `.gsd/quick/plan-sua-nut-dang-nhap/.plan.md`. Thư mục được tạo khi cần
(`ProtocolPaths::ensure_dir` đã có; xác nhận mọi chỗ ghi file giao thức đều gọi nó, và gọi thêm nếu thiếu).

Quy tắc ưu tiên:
- Khóa `protocol_dir` **có mặt** (kể cả chuỗi rỗng `""`) = giá trị người dùng tự đặt, thắng, giữ nguyên hành vi cũ
  (`""` = root). Chỉ khóa **vắng mặt** mới dùng mặc định mới.
- Chọn `.planning` hay `.gsd` chỉ dựa vào việc `.planning/` có tồn tại, KHÔNG dựa vào `.gsd/`, để kết quả
  không đổi sau khi chương trình tự tạo `.gsd/`.

## Phạm vi file
- `agent-brain/src/protocol_paths.rs`: thêm hàm thuần `default_protocol_dir(root: &Path) -> String` và hằng cho hai mẫu.
  Giữ nguyên `paths_in`, `validate_protocol_dir`, `protocol_dir_of_value` (hoặc thêm bản nhận `root`).
- `agent-brain/src/projects.rs` (~dòng 285, `Project::from_value`): nếu mục không có khóa `protocol_dir` thì
  `protocol_dir = default_protocol_dir(<thư mục gốc dự án đã expand_tilde>)`; có khóa thì như cũ.
- `agent-brain/src/knowledge.rs` (`find`, `CANDIDATES`): thư mục `.gsd/` chỉ có `quick/` (do chương trình tạo) KHÔNG
  được coi là thư mục tri thức, nếu không dự án đang dùng `.agent-brain/` sẽ đổi sang `.gsd/` rỗng. Quy tắc:
  `.gsd` chỉ tính khi chứa ít nhất một mục khác ngoài `quick`. Thêm test: `.gsd/quick` rỗng + `.agent-brain` có thật
  → `find` trả `.agent-brain`; `.gsd` có `README.md` → vẫn trả `.gsd`.
- Kiểm tra (đọc, sửa nếu cần) các nơi dùng đường dẫn file giao thức để chắc chắn chúng chấp nhận thư mục nằm
  trong `.gsd/` / `.planning/`: `src/guard.rs` và `src/planfile.rs` (dòng ~1005 cấm cấp quyền ghi `.planning`; chỉ
  áp cho phần "Thư mục được ghi" do người dùng khai, đừng làm lỏng nó), `src/workspace.rs` (`resolve_protocol_conflicts`,
  `keep_base` dựa vào `protocol_dir` khác rỗng: mặc định mới khác rỗng nên giữ bản của nhánh gốc; xác nhận hợp lý),
  `src/tasks.rs` (`current_task_in`), `src/roles/{dev,qa,review}.rs`, `src/fastpath.rs`, `src/autopilot/*`,
  `src/dashboard/api_projects.rs` (hiển thị `protocol_dir`; PUT `/protocol-dir` giữ nguyên).
- Test mới/đã có đụng tới Project dựng từ map với đường dẫn tạm: sửa cho khớp mặc định mới, không làm yếu assertion.
Không đụng: `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` chưa tracked, `~/.claude-orca`, frontend `assets/`.

## Các bước
1. Đọc `protocol_paths.rs`, `projects.rs` (from_value), `knowledge.rs`, `workspace.rs` (đoạn 299-400), `tasks.rs`,
   `guard.rs`, và grep mọi chỗ gọi `protocol_paths::paths`, `paths_in`, `for_branch`, `current`, `ensure_dir`, `PLAN_FILE`.
2. Thêm `default_protocol_dir` + test đơn vị (có `.planning` → `.planning/quick/plan-{slug}`; không có → `.gsd/quick/plan-{slug}`;
   `.planning` là symlink → coi như không có; `.gsd` có sẵn và không có `.planning` → `.gsd/quick/plan-{slug}`).
3. Sửa `Project::from_value`: khóa vắng → mặc định mới; khóa có (kể cả `""`) → giữ. Test cả ba trường hợp.
4. Sửa `knowledge::find` như trên + test.
5. Rà các nơi ở mục "Phạm vi" để chắc agent (dev/QA/review) ghi/đọc được file ở thư mục mới: nếu guard/permission chặn
   ghi vào `.gsd/quick/**` hoặc `.planning/quick/**`, mở đúng đường dẫn của task đó (qua `ProtocolPaths`), không mở rộng cả `.gsd/**`.
6. Một test tích hợp: dựng repo tạm không có `.planning`, project không có `protocol_dir`, tạo task, kiểm tra
   `.plan.md` được kỳ vọng ở `.gsd/quick/plan-<slug>/.plan.md` và thư mục được tạo; root không có `.plan.md`.
7. Commit từng nhiệm vụ, tiền tố `feat(agent-brain):`. Không `git add -A`, không `--no-verify`.

## Tiêu chí nghiệm thu
- Dự án không khóa `protocol_dir`, không có `.planning` → file giao thức ở `.gsd/quick/plan-<slug>/`, root sạch.
- Dự án có `.planning/` → `.planning/quick/plan-<slug>/`.
- Dự án có `"protocol_dir": ""` hoặc giá trị riêng → hành vi cũ, không đổi.
- `.gsd/` tự tạo không làm đổi thư mục tri thức của dự án đang dùng `.agent-brain/`.
- Dashboard hiển thị `protocol_dir` hiệu lực; PUT `/api/projects/{id}/protocol-dir` vẫn hoạt động.
- Không còn test nào bị yếu đi hoặc bỏ qua.

## Lệnh kiểm tra (chạy trong `agent-brain/`, từ worktree)
- `cargo build --release` không lỗi, không warning mới
- `cargo test` (cả bộ; nếu quá lâu thì ít nhất `--lib` và mọi test file có `protocol`, `project`, `knowledge`, `workspace`, `dashboard_projects`, `autopilot`, `roles`)
- `bash spikes/lib/scan-secrets.sh` ở root repo

## Báo cáo cuối
Liệt kê: file đã đổi, mã commit, nhánh/worktree, kết quả test, các điểm lệch plan.
Không merge, không push, không dừng/khởi động lại `agent-brain serve`, không thay `~/.local/bin/agent-brain`.
