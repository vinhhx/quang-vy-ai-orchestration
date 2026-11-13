# Tóm tắt thực hiện: PHP Worktree Env & Code Anchor (worktree-php-env)

## Thông tin commit và HEAD
- **HEAD trước khi bắt đầu**: `d70b7242e5c8d3f2fad3067789bb46a073a2f0b4`
- **HEAD sau khi hoàn thành**: `39415ba2759aa0c656563bfd09ce9cb3063c50cb` (sau Mục 3)
- **Chuỗi commit**:
  1. `f30064460e2a327d4424bc336ddc76ebcfbcef5e`: `workspace: copy vendor vào worktree của dự án PHP`
  2. `ae7366cd48cf7e59f6fdeb402129c5edd70cdc5b`: `container: chạy lệnh của task worktree trong container tạm cùng image`
  3. `39415ba2759aa0c656563bfd09ce9cb3063c50cb`: `review/qa: tính code từ plan đầu tiên của task`
  4. (Commit hiện tại): `docs: tóm tắt worktree-php-env`

## Danh sách file đã sửa theo mục

### 1. Copy `vendor` vào worktree của dự án PHP (`f300644`)
- `agent-brain/src/projects.rs`: Thêm cấu hình `copy_vendor: bool` (mặc định `true`) trong `WorkspaceOpts`, parse từ khóa `"copy_vendor"`, kèm unit tests parsing và defaults.
- `agent-brain/src/workspace.rs`:
  - Khái quát hóa `ensure_patterns_excluded` cho file `.git/info/exclude` dùng chung cho cả `node_modules` và `vendor`.
  - Triển khai `copy_vendor(project_root, worktree)`: xử lý kiểm tra `composer.json` và thư mục `vendor/` thật ở repo chính, xóa symlink cũ nếu có, sao chép đệ quy qua `cp -a --reflink=auto`, đo dung lượng `du -sm` và cảnh báo nếu vượt quá 1 GiB.
  - Cập nhật `create_task_worktree_opts`, `add_worktree`, `ensure_task_root` và bổ sung unit test `copy_vendor_rules`.
- `agent-brain/src/roles/lead.rs`: Truyền `proj.workspace.copy_vendor` vào `ensure_task_root`.

### 2. Chạy lệnh của task worktree trong container tạm cùng image (`ae7366c`)
- `agent-brain/src/container.rs`:
  - Cập nhật `INSPECT_FORMAT` để lấy `Config.Image` (không lấy `Config.Env` tránh lộ secrets).
  - Cập nhật struct `ContainerInfo` và `Inspected` với `image: String` và `extra_binds: Vec<(PathBuf, String)>`.
  - Thêm enum `ContainerTarget`: `Exec` và `Ephemeral { worktree: PathBuf }`.
  - Hàm `decide()` phát hiện chế độ container tạm khi `cwd` nằm trong task worktree và container mount repo chính; cảnh báo rõ ràng tới người dùng.
  - Tạo shims với `docker run --rm -i`, `--network "container:<name>"`, `-v "<wt>:<wt>"`, mount `.env` ở chế độ read-only nếu file tồn tại (không copy vào worktree, không in nội dung), gán nhãn `--label agent-brain.run=<run_id>`.
  - Hàm `cleanup_ephemeral_containers(docker_bin, run_id)` tìm và dọn sạch các container tạm còn sót qua nhãn `agent-brain.run=<run_id>` (`docker ps -q --filter ...` và `docker rm -f`).
  - Thêm unit test `ephemeral_mode_detection_and_shims`.
- **Nơi dọn container ở mục 2.5**:
  - `agent-brain/src/roles/mod.rs`: Gọi `cleanup_ephemeral_containers` trong `finish_run` (nơi ghi `ended_at`, kết thúc run).
  - `agent-brain/src/runner.rs`: Gọi `cleanup_ephemeral_containers` trong lệnh `cmd_kill` (khi run bị huỷ/kill bằng lệnh).

### 3. Neo "có code sau plan" ở plan commit đầu tiên của task (`39415ba`)
- `agent-brain/src/gitops.rs`:
  - Thêm hàm `first_plan_commit_on_branch` trả về commit plan cũ nhất trên nhánh task.
  - Unit test `first_plan_commit_returns_oldest_plan_after_replan`.
- `agent-brain/src/roles/mod.rs`:
  - Thêm hàm dùng chung `code_anchor_for(conn, project_slug, root, task_slug)` cạnh `plan_commit_for`.
  - Unit test `test_task_66_replan_scenario_code_anchor` tái hiện trọn vẹn kịch bản task 66 (base -> plan 1 -> code -> plan 2 -> HEAD).
- `agent-brain/src/roles/review.rs`: Kiểm tra `diff_code_names` và thiết lập `protocol.code_diff_command` dựa trên `code_anchor`.
- `agent-brain/src/roles/qa.rs`: Kiểm tra `diff_code_names` và thiết lập `protocol.code_diff_command` dựa trên `code_anchor`.
- `agent-brain/src/autopilot/mod.rs`: Tính `code_after_plan` dựa trên `code_anchor`.
- `agent-brain/src/dashboard/api_tasks.rs`: Tính `code_after_plan` dựa trên `code_anchor`.
- `agent-brain/README.md`: Bổ sung tiểu mục ngắn trong phần worktree giải thích cơ chế sao chép `vendor` và container tạm cho task PHP worktree.

## Kết quả kiểm tra
- `cargo build --release --offline --locked`: Thành công (Exit code 0).
- `cargo test --offline --locked --no-fail-fast -- --test-threads=1`:
  - **Số test trước**: passed 2107, failed 0
  - **Số test sau**: passed 2111, failed 0 (thêm 4 unit test mới, 0 fail).
- `spikes/lib/scan-secrets.sh`: `scan-secrets: OK` (Exit code 0).

## Lệch plan và lý do
- Không có thay đổi so với thiết kế của plan.
- Nhằm đảm bảo tương thích ngược tuyệt đối với các integration test hiện có ngoài phạm vi file (`tests/housekeeping.rs`, `tests/worktree_node_modules.rs`), giữ lại chữ ký hàm `create_task_worktree(root, branch, base, project_root)` uỷ quyền sang `create_task_worktree_opts(..., false, true)` thay vì thay đổi chữ ký gốc, giúp toàn bộ test suite cũ chạy trơn tru mà không cần can thiệp file ngoài danh mục cho phép.
