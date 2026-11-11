# Plan: màn Dự án: alias, workspace mặc định riêng từng task, nhánh gốc ra bảng

Repo: /mnt/data/IDE/QuangVy-Orchestration (nhánh main), crate `agent-brain/`. Làm trong worktree của bạn.
Nối tiếp commit `40a5e76` (màn Dự án dạng bảng). Đọc `.planning/quick/projects-table-view.plan.md` và diff của `40a5e76` để hiểu cấu trúc hiện tại.

## Yêu cầu của người dùng
1. **Alias**: ô nhập text để **trên**, các chip alias đã có hiển thị **dưới** ô nhập (hiện tại cột này dễ vỡ giao diện).
2. **Workspace**: mặc định là **riêng cho mỗi task** (worktree), không phải dùng chung (shared). Chọn chế độ workspace
   và số task song song chuyển vào **"Nâng cao"** (dòng mở rộng), không còn cột ở bảng chính.
3. **Nhánh gốc của task mới** (`base_branch`) được đưa ra **bảng chính** thành một cột (sửa ngay trên dòng).

Ghi chú để trả lời câu hỏi "song song là cấu hình gì": `max_parallel_tasks` (`src/projects.rs`, khoảng 1–10, mặc định 1) là số task của
dự án được có run/driver chạy cùng lúc, chỉ có tác dụng ở chế độ worktree (ở chế độ shared mỗi dự án chỉ một job).
Trong UI nhãn phải nói rõ điều đó (ví dụ "Số task chạy cùng lúc", có chú thích ngắn), không dùng chữ "Song song" trần.

## Phần A: backend, mặc định workspace = worktree
File: `agent-brain/src/projects.rs`.
- `WorkspaceOpts::default()`: `mode: WorkspaceMode::Worktree` (hiện là `Shared`). Khóa `workspace` có trong projects_map.json
  vẫn thắng (`"shared"` giữ chế độ dùng chung).
- Điều tra trước khi đổi (đọc `src/workspace.rs`, `src/tasks.rs`, `src/orchestrator/*`, `src/dashboard/api_projects.rs`): chế độ worktree cần repo git.
  Dự án không phải repo git, hoặc `kind` không phải code, hoặc `missing`: phải rơi về `shared` (không báo lỗi, không làm hỏng task). Nếu code đã có
  kiểm tra tương tự ở nơi tạo task, dùng lại; nếu chưa, thêm ở một chỗ duy nhất (ví dụ `WorkspaceOpts::effective_mode(root)` hoặc kiểm tra
  khi tạo task) và có test.
- Task đã tạo trước đó giữ nguyên chỗ làm việc của chúng (cột `tasks.workdir`); chỉ task MỚI theo mặc định mới. Xác nhận bằng đọc code + test.
- API `GET /api/projects` trả về `workspace` hiệu lực (đã có); cập nhật test dashboard nào kỳ vọng `shared` mặc định.
- Cập nhật các test trong `projects.rs` (khoảng dòng 1598-1620) và test khác (`grep -rn "WorkspaceMode::Shared\|\"shared\"" src tests`) cho khớp, không làm yếu assertion.

## Phần B: frontend
Files: `agent-brain/assets/dashboard/screens/projects.html`, `agent-brain/assets/dashboard/js/projects.js`, `app.css` (dựng lại bằng
`/mnt/data/IDE/QuangVy-Orchestration/agent-brain/tools/tailwindcss`, đường dẫn tuyệt đối, ra `assets/dashboard/app.css` của worktree; commit `40a5e76` cho thấy cách làm).
- **Cột Alias**: ô input ở trên (full bề rộng ô, Enter/blur lưu như hiện tại), chip alias bên dưới (`flex-wrap`, `break-all`/`min-w-0` để không tràn). Giữ `aria-label`.
- **Bảng chính**: bỏ cột Workspace; thêm cột **"Nhánh gốc"**: input nhỏ sửa ngay trên dòng (placeholder "nhánh hiện tại" khi chưa đặt, vì khi trống
  task mới rẽ từ nhánh đang checkout). Lưu khi Enter/blur bằng đường lưu sẵn có (`saveWorkspace(p)`, PUT cả draft); giữ thông báo lỗi 409/422 và
  nếu lỗi ở field nằm ở dòng mở rộng thì tự mở dòng đó như quy tắc cũ.
  Bỏ ô nhánh gốc khỏi dòng mở rộng (không để hai nơi cùng sửa một giá trị). Hai checkbox git (`hold_project_at_gate`, `auto_checkout_on_approve`) và nút
  "Lưu nhánh và cách duyệt" ở dòng mở rộng: giữ, đổi nhãn nút cho hợp ("Lưu cách duyệt") hoặc giữ nguyên nếu test bám vào.
- **Dòng mở rộng ("Nâng cao")**: thêm nhóm **Workspace**: select `shared`/`worktree` (nhãn rõ: "Riêng cho mỗi task (worktree)" mặc định;
  "Dùng chung repo (shared)"), và ô "Số task chạy cùng lúc" chỉ bật/hiện khi chọn worktree, kèm chú thích. Dùng lại `applyWorkspaceField`/`saveWorkspace`.
- Không đổi hàm gọi API, tên hàm hay id phần tử mà test/JS khác đang dùng khi tránh được.
Không đụng: `src/protocol_paths.rs`, `src/knowledge.rs`, `screens/task.html`, `js/task.js` (executor khác đang sửa), `.planning/STATE.md`, `ROADMAP.md`,
file `.planning/` chưa tracked, `~/.claude-orca`.

## Các bước
1. Đọc `projects.html`, `projects.js`, diff `40a5e76`, `src/projects.rs` (WorkspaceOpts), `src/workspace.rs`, `tests/dashboard_projects.rs`, `tests/dashboard_assets.rs`.
2. Phần A + test; commit `feat(agent-brain): new tasks default to a worktree per task`.
3. Phần B; kiểm tra bằng trình duyệt headless trên instance `agent-brain serve` thử ở cổng khác (KHÔNG đụng cổng 7437 của người dùng, dừng sau khi xong, dừng cả Chrome): alias nhập trên chip dưới ở bề rộng 1440px và 480px không tràn; cột Nhánh gốc sửa/lưu được; Workspace và số task chạy cùng lúc nằm trong Nâng cao và lưu được;
   dự án mới không khóa `workspace` hiện "worktree"; dự án không phải repo git hiện/dùng shared. Commit `feat(dashboard): projects table shows base branch, alias input above chips, workspace under advanced`.
4. Không `git add -A`, không `--no-verify`; mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Tiêu chí nghiệm thu
- Dự án chưa có khóa `workspace` trong projects_map.json: task mới chạy trong worktree riêng; dự án đặt `"workspace": "shared"` giữ shared; không phải repo git thì shared, không lỗi.
- Task đã có không bị di chuyển chỗ làm việc.
- Alias: input trên, chip dưới, không vỡ ở 480px.
- Bảng chính có cột Nhánh gốc sửa được; không còn cột Workspace; Workspace + số task chạy cùng lúc ở Nâng cao; nhãn giải thích đúng nghĩa.
- Mọi API/lỗi 409/422 như trước; a11y và dark mode giữ nguyên.

## Lệnh kiểm tra (trong `agent-brain/`, từ worktree)
- `cargo test` toàn bộ nếu thời gian cho phép; tối thiểu `--lib` và `--test dashboard_assets --test dashboard_projects --test project_read_sources` cùng mọi test file nhắc tới `workspace`/`worktree` (`grep -l`).
- `cargo build --release`
- `bash spikes/lib/scan-secrets.sh` ở root repo

## Báo cáo cuối
File đổi, mã commit, nhánh/worktree, kết quả test và kiểm tra trình duyệt, điểm lệch plan. Không merge, không push, không dừng/khởi động lại `serve`, không thay `~/.local/bin/agent-brain`.
