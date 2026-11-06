# Plan: agy run kết thúc ngay sau sự kiện `result`; lệnh và file agy dùng phải nằm trong quyền của plan

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (đã kiểm tra, 2026-11-06, dự án `srt`, task 11 `tao-video-phong-su-chien-tranh-nga-va-ukraina`)

### Lỗi 1 — agy treo sau khi đã trả `result`, plan bị mất
- Run 54 (Lead, engine `agy`): `stream.jsonl` có sự kiện `{"event":"result","result":{"status":"SUCCESS","response":"# Plan: …"}}` lúc 01:36:31 (response ~10 KB, plan đầy đủ).
- Tiến trình `agy` không thoát: con của nó là `npm exec chrome-devtools-mcp@latest` (MCP server agy tự bật, `agy mcp list` → `chrome-devtools-mcp stdio enabled npx -y chrome-devtools-mcp@latest`) giữ nó sống.
- `runner::supervise` (`src/runner.rs`, vòng `loop { tokio::select! … }` ~dòng 477–560) chỉ thoát khi `child.wait()` xong, hoặc khi hết `req.timeout` → `StopReason::Timeout` → `RunStatus::TimedOut`.
- `roles/lead.rs` ~dòng 543: `RunStatus::TimedOut` → `Err("Lead chạy quá thời gian …")`, không ghi `result.response` vào `.plan.md`. 21 phút chờ vô ích, plan đầy đủ bị bỏ.

### Lỗi 2 — Dev chạy bằng agy không bị giới hạn bởi quyền trong plan
- Với engine Claude, quyền của plan (`## Lệnh được chạy`, `## Thư mục được đọc/ghi`, bảng Thay đổi) được áp bằng hook `PreToolUse` → `agent-brain guard --policy <run_dir>/policy.json` (`src/guard.rs`, `decide`).
- agy KHÔNG đọc `settings.json` của Claude, nên hook không chạy. Run 56 (Dev, agy, `--mode accept-edits`) đã chạy `python3 -c '…'` và `cat << 'EOF' > /tmp/generate_assets.py`, hai dạng lệnh mà `planfile::grant_command_error` và guard đều cấm. Không có gì phát hiện hay chặn.
- `stream.jsonl` của agy có đủ thông tin công cụ trước khi chạy: `{"event":"step_update","step_update":{"step_index":N,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"…"}}}}`. Các tên công cụ đã thấy trong log thật: `read_file` (`Path`), `run_command` (`CommandLine`, có thể có `Cwd`), `list_dir` (`DirectoryPath`), ghi file (`write_to_file`/`replace_file_content`/`multi_replace_file_content`, tham số `TargetFile`). Kiểm lại tên chính xác trong `~/.local/share/agent-brain/runs/srt/tao-video-phong-su-chien-tranh-nga-va-ukraina/*-dev/stream.jsonl` (chỉ đọc, không sửa).

## Mục tiêu
1. Một run agy đã phát sự kiện `result` thì kết thúc trong tối đa `AGY_AFTER_RESULT_GRACE` (mặc định 15 giây): runner dừng cả cây tiến trình (đúng cách S4, dùng `spawn_stop` có sẵn), và run được coi như agy đã tự thoát (không phải `TimedOut`, không phải `Killed`).
2. Nếu run agy vẫn bị `TimedOut` nhưng trước đó đã có `result` với `status == "SUCCESS"` và response không rỗng, thì vai Lead/plan-review/code-review vẫn dùng response đó (giống run thành công) và in một dòng thông báo.
3. Mỗi bước công cụ của agy Dev (và Dev chế độ sửa) được kiểm bằng CHÍNH `guard::decide` với policy của run, ngay khi thấy `step_update` ACTIVE có `tool_info`:
   - vi phạm → in lý do, dừng run ngay (dùng đường kill có sẵn), outcome `protocol_violation`, autopilot dừng như với vi phạm giao thức hiện có;
   - không vi phạm → không làm gì.
4. Không đổi hành vi của engine Claude/DeepSeek/dsh.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/runner.rs` | Mục A |
| 2 | `agent-brain/src/roles/mod.rs` | Mục B, C |
| 3 | `agent-brain/src/stream.rs` | Mục C (chỉ thêm hàm đọc `tool_info` từ dòng agy, nếu chưa có) |
| 4 | `agent-brain/src/guard.rs` | Mục C (chỉ thêm hàm `pub fn agy_tool_payload(...)`) |
| 5 | `agent-brain/src/roles/lead.rs` | Mục B (chỉ nhánh `RunStatus::TimedOut`) |
| 6 | `agent-brain/src/roles/review.rs` hoặc file chứa nhánh TimedOut của plan-review/code-review (tìm bằng grep `RunStatus::TimedOut` trong `src/roles/`) | Mục B |
| 7 | Test: `agent-brain/tests/runner_kill.rs`, `agent-brain/tests/engine_agy.rs`, `agent-brain/tests/engine_guard.rs`, test unit trong `guard.rs`/`stream.rs` | Mục Test |

Không sửa: `config.json` người dùng, `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, cấu hình MCP của agy, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`, dự án `srt`.

### A. `src/runner.rs` — dừng sau `result`
1. `RunRequest`: thêm `pub finish_after_result: Option<Duration>` (None = hành vi cũ). Sửa mọi chỗ dựng `RunRequest` (grep `RunRequest {`) để gán `None`, trừ chỗ ở Mục B.
2. `RunObserver`: thêm phương thức có thân mặc định `fn result_seen(&self) -> bool { false }` (không phá các impl khác như `orchestrator/triage.rs`).
3. Trong vòng `select!`: thêm nhánh hẹn giờ. Khi `req.finish_after_result.is_some()`, `stopping.is_none()` và `obs.result_seen()` lần đầu thành `true`, đặt một `tokio::time::sleep(grace)` (pin, chỉ một lần). Khi nó nổ mà con chưa thoát:
   - `stopping = Some(StopReason::AfterResult)` (biến thể mới), `obs.on_notice("agy đã trả kết quả nhưng chưa thoát sau {grace}s — dừng cây tiến trình")`;
   - `kill_task = Some(spawn_stop(root, req.kill_grace, force.clone(), req.container_stop.clone()))`.
   - Lưu ý: `obs.result_seen()` chỉ đổi sau `obs.on_line`, nên kiểm điều kiện ngay sau `obs.on_line(&line)` trong nhánh `rx.recv()`.
4. Map trạng thái cuối: `Some(StopReason::AfterResult) => RunStatus::Succeeded` (giữ `exit_code` thật để log). Signal/Timeout giữ nguyên. Nếu trong lúc chờ grace có SIGINT/SIGTERM/timeout thì chúng thắng như cũ (vì `stopping` đã set thì nhánh hẹn giờ không chạy).

### B. `src/roles/mod.rs` và các vai đọc response
1. Observer trong `execute_role_staged` (impl `RunObserver` ~dòng 517): `result_seen()` trả `self.summary.result_seen()` (hàm có sẵn ở `stream.rs:285`).
2. Khi dựng `RunRequest` cho engine có `stream == StreamFormat::Agy`: `finish_after_result: Some(Duration::from_secs(AGY_AFTER_RESULT_GRACE_SECS))`, hằng `pub const AGY_AFTER_RESULT_GRACE_SECS: u64 = 15;` đặt trong `roles/mod.rs`. Engine khác: `None`.
3. Sau `supervise` (đoạn ~dòng 655–670 có xử lý `agy_print_timeout_hit`): thêm quy tắc, đặt TRƯỚC kiểm quota:
   - nếu `agy_stream && outcome.status == TimedOut && obs.summary.result_seen() && obs.summary.agy_status.as_deref() == Some("SUCCESS") && agy_response không rỗng (trim)` → `outcome.status = Succeeded`, `obs.on_notice("Run quá thời gian nhưng agy đã trả kết quả SUCCESS trước đó — dùng kết quả này")`.
   - Không áp quy tắc này khi `agy_print_timeout_hit(...)` đúng (response rỗng của print-timeout vẫn là TimedOut).
4. `roles/lead.rs` và vai review: không cần sửa nếu (3) đã đổi status trước khi tới `match role_run.outcome.status`. Chỉ sửa nếu đọc code thấy status được lấy ở chỗ khác. Ghi rõ trong tóm tắt.

### C. Kiểm từng bước công cụ của agy Dev bằng guard
1. `guard.rs`: thêm `pub fn agy_tool_payload(tool_name: &str, params: &serde_json::Map<String, serde_json::Value>, root: &Path) -> Option<serde_json::Value>` trả payload dạng hook Claude để đưa vào `decide`:
   - `run_command` → `{"tool_name":"Bash","tool_input":{"command": CommandLine},"cwd": Cwd hoặc root}`;
   - `read_file`/`view_file` → `Read` với `file_path` = `Path`/`AbsolutePath`;
   - `list_dir` → `Glob` (hoặc `Read` nếu guard không có Glob theo thư mục) với đường dẫn `DirectoryPath`;
   - `write_to_file`/`replace_file_content`/`multi_replace_file_content` → `Write`/`Edit` với `file_path` = `TargetFile`;
   - `grep_search`/`find_by_name` → `Grep`/`Glob` với đường dẫn tìm;
   - công cụ không biết (browser_*, ask_*, …) → `Some` payload với `tool_name` nguyên gốc, để `decide` từ chối theo mặc định của nó. Nếu `decide` cho phép tool lạ, thì trả `None` cho các tool chỉ hỏi người dùng (`ask_question`, `ask_permission`, `ask_custom_permission`) và từ chối tất cả `browser_*`. Ghi quyết định trong tóm tắt.
   Tên tham số chính xác lấy từ log thật (xem Sự cố gốc). Giữ ánh xạ trong MỘT bảng hằng.
2. Observer (`roles/mod.rs`): nếu vai là `dev` (gồm chế độ fix), engine agy, và run có `policy.json` (run Dev luôn ghi policy, xem `guard::write_hook_settings*`), thì nạp `Policy` một lần lúc bắt đầu. Trong `on_line`, với mỗi `step_update` có `state == "ACTIVE"` và `tool_info` (kiểm mỗi `step_index` một lần), dựng payload và gọi `guard::decide`. Nếu bị từ chối:
   - đánh dấu `violation = Some(lý do)`, in `"agy Dev dùng công cụ ngoài quyền của plan: <tool> <tham số rút gọn 200 ký tự> — <lý do>"`;
   - yêu cầu dừng run qua cơ chế có sẵn (`kill_requested()` trả true, hoặc cờ tương đương mà runner đang đọc). Đọc code runner để chọn đúng đường; KHÔNG dùng `pkill`/`killall`.
3. Sau `supervise`: nếu `violation.is_some()` → outcome run `protocol_violation`, trả cùng mã thoát/lỗi như vi phạm giao thức Dev hiện có (tìm `protocol_violation` trong `roles/dev.rs`), để autopilot dừng như với vi phạm khác. Ghi lý do vào `runs.error` nếu đường hiện có hỗ trợ.
4. Lead và reviewer chạy agy ở chế độ chỉ đọc (không có cờ quyền, ghi file bị soft-deny), KHÔNG áp Mục C cho chúng.

## Không được đụng tới
- Hành vi engine Claude/DeepSeek/dsh, hook Claude, `guard::decide` hiện có (chỉ thêm hàm mới).
- Cờ quyền agy (`--mode accept-edits` cho Dev), không thêm `--dangerously-skip-permissions`.
- Cấu hình MCP của agy trên máy người dùng.
- Không `git add -A`, không `--no-verify`.

## Test (bắt buộc)
Dùng giả lập, không gọi agy thật:
- (a) `runner`: chương trình giả in một dòng `result` rồi ngủ 600 giây và sinh một tiến trình con cũng ngủ; với `finish_after_result = Some(1s)` và `timeout = 60s` → run kết thúc trong < 10s, `status == Succeeded`, không còn tiến trình sót (dùng cách kiểm tiến trình sót đang có trong `tests/runner_kill.rs`). Với `finish_after_result = None` → hành vi cũ (TimedOut khi hết timeout ngắn).
- (b) `engine_agy`/`cli_lead`: Lead agy giả in `result` SUCCESS có response plan hợp lệ rồi treo → `.plan.md` được ghi từ response, run `succeeded`.
- (c) TimedOut + result SUCCESS response không rỗng → coi là thành công; TimedOut + print-timeout (response rỗng, dòng stderr print-timeout) → vẫn `timed_out`.
- (d) `guard::agy_tool_payload` + `decide`: `run_command` `python3 -c 'x'` bị từ chối; `run_command` khớp mẫu trong `## Lệnh được chạy` được phép; `cat << 'EOF' > /tmp/a.py` bị từ chối; `write_to_file` vào file ngoài bảng Thay đổi bị từ chối; `read_file` file trong `## Thư mục được đọc` được phép; `browser_navigate` bị từ chối.
- (e) Dev agy giả phát `step_update` ACTIVE `run_command` bị cấm → run bị dừng, outcome `protocol_violation`; Dev agy giả chỉ dùng công cụ hợp lệ → không ảnh hưởng.
- (f) Toàn bộ test cũ pass.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
git diff --stat <HEAD trước khi chạy>..HEAD   # chỉ các file trong bảng Thay đổi
```

## Tiêu chí hoàn thành
1. Run agy treo sau `result` kết thúc trong ≤ 15s + kill grace, status `succeeded`, plan Lead được ghi.
2. Lệnh/file agy Dev dùng ngoài quyền của plan bị phát hiện ngay ở bước ACTIVE, run bị dừng, outcome `protocol_violation`.
3. Engine khác không đổi hành vi; tất cả test pass; fmt/clippy sạch.

## Commit
Mỗi mục A, B, C một commit, `feat(agent-brain): …` hoặc `fix(agent-brain): …`, chỉ add đúng file. Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (đặc biệt tên tool/tham số agy thật và cách dừng run đã chọn), kết quả từng lệnh kiểm tra, phần chưa kiểm (chưa chạy với agy thật).
