# Summary: agy Dev đọc ngoài danh sách chỉ cảnh báo, không dừng run (agy-soft-read)

Plan: `.planning/quick/agy-soft-read.plan.md`.
Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`, base HEAD `9c9573a`.

## Files changed

- `agent-brain/src/guard.rs`:
  - Thêm `pub fn agy_soft_read(policy: &Policy, tool_name: &str, params: &serde_json::Map<String, serde_json::Value>) -> Option<String>`.
  - Thêm helper `fn is_safe_read_path(policy: &Policy, raw_path: &str) -> bool`: chuẩn hóa NFC, giải quyết symlink bằng `realpath_nfc`, kiểm tra nằm trong `root` hoặc `source_roots` và không thuộc đường dẫn cấm (`grant_denied_rel`).
  - Kiểm tra an toàn cho `Read`, `Glob`, `Grep` và `Bash`/`run_command` (cwd hợp lệ, không chứa ký tự/toán tử shell nguy hiểm, không có biến môi trường đứng trước, từ đầu thuộc danh sách lệnh chỉ đọc cho phép, `find` không có cờ cấm, đối số được kiểm tra đường dẫn an toàn ngoại trừ pattern đầu tiên của grep/rg).
  - Bổ sung 14 unit test (`agy_soft_read_*`) bao phủ đầy đủ tất cả các trường hợp theo plan: file tồn tại, file không tồn tại, file `.env.*`, file ngoài root (`/etc/passwd`), symlink trỏ ra ngoài root, liệt kê thư mục con, lệnh `ls`, `cat .env.*`, lệnh có `;`, `find -delete`, `grep -rn`, lệnh ghi/xóa (`rm`, `write_file`).
- `agent-brain/src/roles/mod.rs`:
  - Mở rộng struct `Observer`: thêm trường `soft_reads: Vec<String>`.
  - Cập nhật `Observer::check_agy_tool`: khi `decision.allow == false`, gọi `crate::guard::agy_soft_read(policy, &tool, &params_clone)`. Nếu là thao tác đọc an toàn, phát thông báo cảnh báo qua `term::emit`, ghi nhận vào `self.soft_reads` (≤200 ký tự) và `return` mà không đặt `violation`/`stop_reason` (không dừng run).
  - Cập nhật khởi tạo `Observer` trong `run_task_role`: khởi tạo `soft_reads: Vec::new()`.
  - Cập nhật ghi metrics trong `run_task_role`: đưa `agy_soft_reads` (tối đa 50 mục) vào `runs.metrics_json` khi luồng là `Agy`.

## Commits

1. `4781731`: `guard: agy_soft_read cho đọc an toàn trong worktree`
2. `06f2056`: `roles: agy Dev đọc ngoài danh sách chỉ cảnh báo, không dừng run`

## Khoá tham số agy đã dùng

Được trích xuất và ánh xạ qua `agy_tool_payload` kết hợp trực tiếp trong `guard.rs`:
- `run_command` -> `Bash`: `"CommandLine"`, `"Command"` (cùng key `"Cwd"` để kiểm tra thư mục chạy lệnh)
- `read_file`, `view_file` -> `Read`: `"Path"`, `"AbsolutePath"`, `"TargetFile"`, `"File"`
- `list_dir`, `find_by_name` -> `Glob`: `"DirectoryPath"`, `"Path"`, `"SearchDirectory"`, `"SearchPath"`
- `grep_search` -> `Grep`: `"SearchPath"`, `"SearchDirectory"`, `"Path"`, `"DirectoryPath"`

## Lệch plan

Không có lệch logic so với plan.

## Verify

- `cargo build --release --offline --locked`: OK (Finished release profile [optimized] target(s) in 1m 24s)
- `cargo test --offline --locked`: OK (14 unit test mới của `agy_soft_read` đều pass, toàn bộ test suite pass, 0 failed)
- `bash ../spikes/lib/scan-secrets.sh`: OK (`scan-secrets: OK`)
