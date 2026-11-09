# Plan test-commands-quota-stops: QA/Dev chạy được lệnh kiểm tra của dự án + dọn quota_stops cũ (2026-11-09)

## Bối cảnh (task 38, dự án `cms`)
- QA (engine claude) chỉ có `--allowedTools` = Read/Grep/Glob + git chỉ đọc + ghi `.qa-report.md` (`src/roles/qa.rs:5-10`, `:243`). Lệnh `php artisan test` trong mục "Cách kiểm tra" của plan bị Claude Code từ chối ("This command requires approval", run 197 `permission_denials`), dù guard hook ghi `allow`. QA vì vậy luôn kết luận "Cần sửa" vì "chưa có bằng chứng test".
- Dev ở vòng sửa (`--fix`) không có lệnh nào ngoài `git add/commit` (grants của plan chỉ có khi plan khai `## Tạo sản phẩm`, `src/roles/dev.rs:154-200`), nên Dev ghi vướng mắc rồi dừng (run 198). Vòng lặp QA→fix kẹt mãi.
- Bảng `quota_stops` chỉ bị xoá khi auto-resume chạy hoặc khi task không có proposal `started` (`src/orchestrator/scheduler.rs:427-501`). Khi người dùng tự chạy lại (nút Chạy QA / Chạy thêm 1 vòng / Telegram) mà `auto_resume_quota=false`, dòng cũ nằm lại mãi ⇒ màn Queue (`/api/board` → `quota_stops`, `src/dashboard/api_intake.rs:657`) vẫn hiện "Hết quota".

## Mục tiêu
1. Mỗi dự án khai được **lệnh kiểm tra** (`test_commands`) trong `projects_map.json`. Run QA và run Dev (kể cả `--fix`) của dự án đó được phép chạy đúng các lệnh này; QA được báo trong đầu vào là chạy được các lệnh nào.
2. Dòng `quota_stops` của task bị xoá khi run mới nhất của task không còn là `quota` (và task không còn `auto_stop_kind = 'het_quota'`).

Không đổi hành vi khi dự án không khai `test_commands`.

## Phạm vi file (chỉ sửa trong)
- `agent-brain/src/projects.rs`
- `agent-brain/src/roles/qa.rs`
- `agent-brain/src/roles/dev.rs`
- `agent-brain/src/orchestrator/scheduler.rs`
- `agent-brain/src/db.rs` (chỉ khi cần thêm hàm truy vấn)
- `agent-brain/tests/cli_qa.rs`, `agent-brain/tests/cli_dev.rs`, `agent-brain/tests/cli_fix.rs` (thêm test), test trong module (`#[cfg(test)]`) của các file trên
- `agent-brain/README.md` (một đoạn mô tả khoá `test_commands`)

Không đụng: `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` chưa track, `~/.claude-orca`, `~/.config/agent-brain/**` (cấu hình thật — Claude sẽ tự thêm khoá cho `cms` sau khi review), binary đã cài `~/.local/bin/agent-brain`.

## 0. Trước khi sửa
Ghi HEAD và số nền:
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
git rev-parse HEAD
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"
```

## 1. Khoá `test_commands` (`src/projects.rs`)
- `pub const TEST_COMMANDS_KEY: &str = "test_commands";`, `pub const MAX_TEST_COMMANDS: usize = 10;`
- `pub fn parse_test_commands(v: Option<&serde_json::Value>) -> Result<Vec<String>, String>`:
  - không có / `null` ⇒ `Ok(vec![])`;
  - phải là mảng chuỗi; mỗi chuỗi `trim()`, bỏ rỗng, bỏ trùng; quá `MAX_TEST_COMMANDS` ⇒ `Err`;
  - mỗi mẫu kiểm bằng `crate::planfile::grant_command_error(pattern)` (cùng luật với "Lệnh được chạy" của plan: denylist chương trình, không ký tự shell…). Có lỗi ⇒ `Err` tiếng Việt nêu mẫu và lý do.
- `pub fn test_commands_of(map: &serde_json::Value, project_id: &str) -> Result<Vec<String>, String>` — tìm entry theo `id` như `container_of` (`src/projects.rs` ~dòng 694), dự án không có trong map ⇒ `Ok(vec![])`.
- Nếu `src/projects.rs` có hàm validate/ghi lại entry dự án (giống cách `container`, `workspace`, `base_branch` được kiểm khi lưu), thêm kiểm `test_commands` vào đó và giữ nguyên khoá khi ghi lại. Không thêm UI dashboard.
- Unit test: thiếu khoá; mảng hợp lệ (`["php artisan test", "php artisan test *"]`); không phải mảng; phần tử không phải chuỗi; mẫu bị từ chối (`"rm -rf *"`, `"bash -c x"`, `"docker ps"`); quá 10 mục; bỏ trùng.

## 2. QA được chạy lệnh kiểm tra (`src/roles/qa.rs`)
Trong hàm chạy QA, ngay sau `let mut allowed = qa_allowed_tools(&protocol);` (~dòng 243), **cả nhánh dry-run lẫn nhánh chạy thật** dùng cùng danh sách:
- `let test_cmds = crate::projects::test_commands_of(&ctx.map, &proj.id)`; `Err(e)` ⇒ `crate::term::emit(&format!("Cảnh báo: {e} — QA chạy không có lệnh kiểm tra"))` và dùng `vec![]` (không làm hỏng run).
- Với mỗi mẫu `c`: `allowed.push(format!("Bash({c})"))`.
- Nếu `test_cmds` không rỗng, nối vào `qa_input` một đoạn (hằng `pub const QA_TEST_COMMANDS_NOTE` hoặc hàm `fn qa_test_commands_note(cmds: &[String]) -> String`):
  ```
  

  Lệnh kiểm tra bạn được chạy (cấu hình của dự án): 
  - `php artisan test`
  - `php artisan test *`
  Chạy các lệnh trong mục Cách kiểm tra của plan nếu khớp các mẫu này; ghi output tóm tắt (số test đạt/lỗi) vào báo cáo. Lệnh không khớp mẫu sẽ bị từ chối — khi đó ghi rõ lệnh nào chưa chạy được, không kết luận Cần sửa chỉ vì lệnh đó.
  ```
- Guard hook của QA (`write_bash_hook_settings`) giữ nguyên: với engine claude, hook để Claude tự quyết lệnh không phải git (đã thấy `allow` trong guard.jsonl run 197). Kiểm lại điều này bằng test ở mục 5; nếu hook thực tế chặn `php …` thì **dừng và báo lại**, không tự nới guard.
- Engine agy/dsh cho QA: ngoài phạm vi (không đổi). Ghi một dòng trong tóm tắt.

## 3. Dev được chạy lệnh kiểm tra (`src/roles/dev.rs`)
Trong hàm chạy Dev, sau đoạn thêm grants của plan (~dòng 559-617) và trước khi viết hook settings:
- Đọc `test_cmds` như mục 2 (lỗi ⇒ cảnh báo, `vec![]`).
- Engine claude: với mỗi mẫu, nếu `Bash({c})` chưa có trong `allowed` thì thêm. Áp dụng cả `--fix`, `--finalize`, tiếp tục sau quota.
- Engine agy: guard của agy Dev chỉ cho git khi policy không có danh sách lệnh (`src/guard.rs:450`). Ở chỗ ghi hook settings (~dòng 809-845):
  - nếu có `dev_grants`: thêm `test_cmds` vào `bash_allow` (tạo `Some(vec)` nếu đang `None`), bỏ trùng;
  - nếu không có `dev_grants` và `test_cmds` không rỗng: gọi `write_hook_settings_grants` với `GuardGrants { bash_allow: Some(test_cmds.clone()), ..Default::default() }` thay cho `write_hook_settings`, các tham số còn lại như nhánh hiện tại.
- Nếu `test_cmds` không rỗng, nối vào đầu vào của Dev một dòng: "Lệnh kiểm tra được chạy (cấu hình của dự án): `…`, `…`. Chạy chúng để tự kiểm trước khi commit." Không đổi các câu hiện có trong `assets/prompts/dev.md`.
- Dev người (human) và dsh: không đổi.

## 4. Dọn `quota_stops` cũ (`src/orchestrator/scheduler.rs`)
Trong `handle_quota_stops`, sau bước (b) ghi quota_stops (~dòng 427) và **trước** bước (c), luôn chạy (không phụ thuộc `auto_resume_quota`):
- Với mỗi dòng của `crate::db::list_quota_stops(conn)?` mà `task_id` **không** nằm trong `candidates` (danh sách task hiện còn dừng vì quota) ⇒ `crate::db::delete_quota_stop(conn, task_id)`.
- Lưu ý `candidates` chỉ gồm task có proposal `started` và `auto_state != 'running'`. Để không xoá nhầm dòng của task đang chạy lại do auto-resume: chỉ xoá khi run mới nhất của task **không** có `outcome = 'quota'` **và** `tasks.auto_stop_kind` khác `'het_quota'` (viết thành một hàm `db::quota_stop_is_stale(conn, task_id) -> BrainResult<bool>` với một câu SQL, hoặc truy vấn trực tiếp trong scheduler). Task đã bị xoá khỏi bảng `tasks` cũng tính là stale.
- Unit/integration test (theo kiểu test hiện có của `scheduler.rs`/`db.rs`): 
  1. task có dòng quota_stops, run mới nhất `outcome='quota'` ⇒ giữ;
  2. thêm một run mới `status='succeeded', outcome='qa:fix'` ⇒ sau `handle_quota_stops` (hoặc hàm dọn tách riêng) dòng bị xoá;
  3. `auto_stop_kind='het_quota'` dù run mới nhất không phải quota ⇒ giữ.

## 5. Test tích hợp
- `tests/cli_qa.rs`: test mới `qa_gets_project_test_commands` — sandbox ghi `projects_map.json` có `"test_commands": ["php artisan test", "php artisan test *"]` cho dự án test (xem cách `Sandbox` ghi map trong `tests/common/mod.rs` ~dòng 280-300). Chạy `run qa`, lấy argv từ `sb.records()`; khẳng định `--allowedTools` chứa `Bash(php artisan test *)` và đầu vào `-p` chứa "Lệnh kiểm tra bạn được chạy". Test thứ hai: không có khoá ⇒ không có `Bash(php` nào trong argv và không có đoạn ghi chú.
- Test mẫu bị từ chối (`"rm -rf *"`): run QA vẫn chạy (exit như bình thường), stderr/stdout có "Cảnh báo", argv không có `Bash(rm`.
- `tests/cli_fix.rs` (hoặc `cli_dev.rs`): Dev `--fix` với `test_commands` ⇒ argv có `Bash(php artisan test *)`.

## 6. README
Thêm vào phần cấu hình dự án của `agent-brain/README.md` một đoạn ngắn: khoá `test_commands`, ví dụ cho Laravel, luật kiểm (như "Lệnh được chạy" của plan), áp dụng cho QA và Dev engine claude (Dev agy qua guard), kết hợp với `container` (lệnh `php` vẫn chạy trong container nhờ shim).

## Kiểm tra (bắt buộc chạy, dán kết quả vào tóm tắt)
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --release
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'   # > nền, failed 0
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # ≤ nền
cargo test --test cli_qa qa_gets_project_test_commands
git diff --stat <HEAD nền>..HEAD    # chỉ các file trong Phạm vi
```
**Không** cài binary vào `~/.local/bin` và không khởi động lại `agent-brain serve` (Claude làm khi serve rảnh).

## Commit
Mỗi mục 1-4 một commit (mục 5-6 gộp vào commit của mục liên quan được), message dạng `fix(agent-brain): …` / `feat(agent-brain): …`, kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Không `git add -A`, không `--no-verify`.

## Tiêu chí nghiệm thu
- Dự án có `test_commands`: argv QA (claude) có `Bash(<mẫu>)` cho từng mẫu + đoạn ghi chú; argv Dev (claude, kể cả `--fix`) có `Bash(<mẫu>)`; policy agy Dev có `bash_allow` chứa các mẫu.
- Dự án không có khoá: argv QA/Dev giống hệt trước (test hiện có không phải sửa kỳ vọng).
- Mẫu bị denylist không bao giờ vào argv/policy.
- `quota_stops` của task đã chạy lại xong bị xoá ở lần quét scheduler kế tiếp, kể cả khi `auto_resume_quota=false`.
- Test + clippy như mục Kiểm tra.

## Tóm tắt phải ghi
File đã sửa, các commit, chỗ lệch plan (và lý do), kết quả các lệnh Kiểm tra, ghi chú QA agy/dsh ngoài phạm vi.
