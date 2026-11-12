# Plan: tự dọn worktree và nhánh của task đã xong, và dọn sớm khi ổ đĩa gần đầy

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (2026-11-12)
Ổ `/mnt/data` đầy 99%. Worktree của task agent-brain chỉ nặng 170-612 MB mỗi cái nhưng không bao giờ tự được dọn; nhánh `task/*` đã merge cũng nằm lại mãi (`task cleanup` giữ nhánh, chỉ `task merge --push` xoá nhánh). Hôm nay phải dọn tay 7 worktree task, 65 nhánh đã merge ở 11 dự án. Người dùng đã chốt luật:
1. Nhánh task ĐÃ merge thì xoá (nếu cần sửa thì tạo nhánh mới từ nhánh gốc). Chỉ `git branch -d`, không bao giờ `-D`.
2. Nhánh CHƯA merge thì KHÔNG bao giờ tự xoá (mất việc); chỉ người dùng quyết.
3. Worktree sạch của task `qa_pass` chưa merge quá 7 ngày: dọn worktree, giữ nhánh.
4. Ổ đĩa còn dưới 20 GB: dọn sớm mọi worktree sạch của task đã xong (nhánh chưa merge vẫn giữ).
5. Không xoá nhánh đang là nhánh gốc (`base_branch`) của một task chưa xong (task xây chồng).

## Hiện trạng (đã đọc code)
- `agent-brain/src/workspace.rs`: `cleanup_refusal(task, auto_state, busy, merged, confirmed)` (~dòng 280): task phải xong (`tasks::is_success_status`) hoặc `auto_state = ended`, không bận, và chưa merge thì cần `confirmed`; `cleanup_task(conn, project, project_root, task, confirmed)` (~dòng 326): kiểm dirty (`gitops::changed_paths`), `gitops::worktree_remove` không `--force`, đặt `workdir = NULL`, KHÔNG xoá nhánh. `merge_base_of(project, task)`, `task_busy`, `worktrees_dir()`.
- `workspace::delete_task_branch` (~dòng 816) chỉ dùng trong luồng `merge --push` và cần `cfg.merge.delete_task_branch`; từ chối khi task còn worktree.
- `agent-brain/src/orchestrator/scheduler.rs`: `schedule_tick(st)` chạy mỗi 5 giây từ `orchestrator::spawn_loop`; đã gọi `handle_env_stops` và `handle_queued_actions` trước khi kiểm `orch.enabled`.
- `agent-brain/src/db.rs`: `tasks_with_workdir(conn, project_row_id)`, `get_task`, `task_auto`, `set_task_workdir`; bảng `tasks` có `branch`, `base_branch`, `workdir`, `status`, `auto_state`, `updated_at`.
- `agent-brain/src/main.rs`: lệnh `task cleanup [--merged] [--yes] [TASK_ID]` (`workspace::cmd_cleanup` ~dòng 1027+).
- `agent-brain/src/config.rs`: `struct Config` (có `run_stall_minutes` ~dòng 1126, khối `merge`), đọc `config.json` với kiểm khoảng giá trị và thông báo lỗi tiếng Việt.

## Luật mới
1. Khoá cấu hình mới `housekeeping` ở cấp trên cùng của `config.json`: `{ "enabled": true, "interval_minutes": 10, "keep_unmerged_days": 7, "delete_merged_branches": true, "min_free_gb": 20 }`. Thiếu khoá hay thiếu cả khối → mặc định như trên. Khoảng hợp lệ: `interval_minutes` 1..=1440; `keep_unmerged_days` 1..=365; `min_free_gb` 0..=2000 (0 = tắt dọn theo dung lượng); sai kiểu hoặc ngoài khoảng → lỗi cấu hình tiếng Việt như các khoá khác.
2. Mỗi `interval_minutes` phút, trong tick của scheduler (chạy cả khi `orchestrator.enabled = false`, giống `handle_queued_actions`), `housekeeping::tick` duyệt các task có `workdir` khác rỗng của dự án chế độ worktree:
   - task đã xong, không bận, worktree SẠCH, nhánh ĐÃ merge vào nhánh gốc → `workspace::cleanup_task(.., confirmed = false)`;
   - task xong, không bận, sạch, nhánh CHƯA merge, `updated_at` cũ hơn `keep_unmerged_days` ngày HOẶC ổ đĩa còn dưới `min_free_gb` → `cleanup_task(.., confirmed = true)` (chỉ dọn worktree, nhánh giữ);
   - mọi trường hợp khác (đang chạy, dirty, task chưa xong, chưa đến hạn): bỏ qua, không lỗi.
3. Xoá nhánh (nếu `delete_merged_branches`): với task xong của dự án chế độ worktree KHÔNG còn worktree (vừa dọn hoặc đã dọn từ trước), nhánh `task/*` hoặc `analysis/*` ghi trong DB còn tồn tại, ĐÃ merge vào nhánh gốc (`gitops::is_merged_into`), không phải nhánh đang checkout, và không có task chưa xong nào dùng nó làm `base_branch` → `git branch -d -- <nhánh>` (không `-D`). Git từ chối thì bỏ qua, ghi một dòng. Chỉ nhánh có trong bảng `tasks`; không đụng nhánh tự tạo của người dùng không có trong DB.
4. Dung lượng trống = dung lượng trống của hệ thống tệp chứa `workspace::worktrees_dir()`, lấy bằng `df -Pk <thư mục>` (chỉ thư viện chuẩn, không thêm dependency). Lỗi đọc → coi là không biết, không dọn theo dung lượng. Hàm nhận dung lượng làm tham số để test.
5. Mỗi lần dọn ghi một dòng `crate::term::emit("Dọn tự động: …")`. Lỗi `cleanup_task` do dirty hoặc git từ chối chỉ ghi một lần cho mỗi (task, lý do) mỗi lần chạy serve (tránh lặp mỗi 10 phút).
6. `agent-brain task cleanup --merged` có thêm cờ `--branches`: sau khi dọn, cũng xoá các nhánh đã merge theo luật 3 (dùng chung hàm với tick). Không đổi hành vi khi không có cờ.
7. Không tự dọn khi `housekeeping.enabled = false`. Không bao giờ dùng `--force`, `-D`, hay xoá thư mục bằng `rm`.

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/config.rs` | `struct Config`, đọc cấu hình | Thêm struct `Housekeeping { enabled, interval_minutes, keep_unmerged_days, delete_merged_branches, min_free_gb }` với mặc định và kiểm khoảng (luật 1), trường `Config.housekeeping`; thêm khoá `housekeeping` vào danh sách khoá cấp trên cùng được phép (nếu có kiểm khoá lạ). Đặt khối mới ở CUỐI danh sách trường của `Config` (một executor khác sửa cạnh `run_stall_minutes`). Test: mặc định, giá trị hợp lệ, ngoài khoảng, sai kiểu. | Cấu hình |
| 2 | `agent-brain/src/orchestrator/housekeeping.rs` (mới) và `agent-brain/src/orchestrator/mod.rs` (một dòng `pub mod`) | `housekeeping::tick(st, conn, now_ms, free_gb: Option<u64>) -> BrainResult<Vec<String>>`, `free_gb_of(dir) -> Option<u64>`, `delete_merged_branches(conn, project, root) -> Vec<String>` | Luật 2, 3, 4, 5. Tái dùng `workspace::cleanup_task`, `merge_base_of`, `gitops::is_merged_into`, `gitops::branch_exists`, `gitops::current_branch`; thêm hàm xoá nhánh dùng chung (nằm trong `housekeeping.rs` hoặc `workspace.rs`, không nhân đôi). Thời điểm chạy gần nhất giữ trong biến tĩnh `OnceLock<Mutex<i64>>`. | Logic dọn |
| 3 | `agent-brain/src/orchestrator/scheduler.rs` | `schedule_tick` | Ngay sau lời gọi `handle_queued_actions`, gọi `housekeeping::tick` nếu đã đến hạn; lỗi chỉ ghi `term::emit`, không làm hỏng tick. | Chạy định kỳ |
| 4 | `agent-brain/src/main.rs` và `agent-brain/src/workspace.rs` | `task cleanup` (`cmd_cleanup`) | Thêm cờ `--branches` (luật 6). | Dọn tay cùng logic |
| 5 | `agent-brain/tests/housekeeping.rs` (mới) | cả file | Dùng thư mục tạm, `git init`, DB tạm, dự án chế độ worktree (mẫu: `tests/worktree_node_modules.rs`, `tests/queued_actions.rs`; đặt `XDG_DATA_HOME` về thư mục tạm). Ca kiểm: (a) task xong + nhánh đã merge + sạch → worktree bị dọn, nhánh bị xoá; (b) task xong, chưa merge, `updated_at` mới → giữ cả worktree và nhánh; (c) chưa merge, quá `keep_unmerged_days` → worktree dọn, nhánh giữ; (d) chưa merge, mới, dung lượng giả lập dưới `min_free_gb` → worktree dọn, nhánh giữ; (e) worktree dirty → giữ; (f) task đang có run hoặc autopilot → giữ; (g) nhánh là `base_branch` của task chưa xong → worktree dọn nhưng nhánh giữ; (h) nhánh đang checkout → giữ; (i) `delete_merged_branches = false` → nhánh giữ; (j) `enabled = false` → không làm gì; (k) task chưa xong → không đụng; (l) nhánh không có trong DB → không đụng; (m) `task cleanup --merged --branches` xoá nhánh đã merge, không có cờ thì không xoá; (n) cấu hình: giá trị ngoài khoảng bị từ chối. Test không bao giờ dùng `~/.local/share/agent-brain` hay repo thật. | Chốt hành vi |
| 6 | `agent-brain/README.md` | mục cấu hình / worktree | Mô tả khối `housekeeping`, từng khoá, luật "nhánh chưa merge không bao giờ tự xoá", cờ `--branches`, và cách tắt. | Tài liệu |

## Không được đụng tới
- `workspace::cleanup_refusal` và `cleanup_task` (chỉ gọi, không đổi hành vi), `merge_task`, `merge_push_task`, `delete_task_branch`, logic autopilot, `claim.rs`, `runner.rs`, `roles/*`, `dashboard/*`, `telegram/*`, `db.rs` (chỉ đọc; nếu cần một truy vấn mới thì thêm hàm nhỏ, ghi vào summary).
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db, runs, worktrees thật; serve đang chạy: test dùng thư mục tạm), `/mnt/data/source_codes/**`, `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- KHÔNG thay binary `agent-brain` đang chạy. KHÔNG dùng `pkill`/`killall`/kill theo tên tiến trình (CLAUDE.md của repo); chỉ dừng tiến trình do bạn chạy bằng PID bạn đã ghi. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.
- Ổ đĩa `/mnt/data` từng đầy: với MỌI lệnh cargo đặt `export CARGO_TARGET_DIR=/mnt/data/IDE/QuangVy-Orchestration/agent-brain/target` (thư mục build chung của main; không tạo `target/` trong worktree, không xoá thư mục đó). Một executor khác chạy song song và cũng dùng thư mục này: cargo tự khoá, chỉ có thể phải chờ. Dùng tên tệp riêng cho đầu ra tạm.

## Cách kiểm tra
Trong worktree của bạn, thư mục `agent-brain/`, với `CARGO_TARGET_DIR` như trên:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --lib config 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test housekeeping 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test orchestrator_scheduler --test queued_actions --test worktree_node_modules 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(orchestrator/(housekeeping|scheduler)|config|workspace)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test chập chờn đã biết (chạy lại riêng và ghi vào summary nếu là test duy nhất fail): `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. Task xong, nhánh đã merge, worktree sạch: worktree và nhánh bị dọn trong vòng một chu kỳ, không cần lệnh tay.
2. Nhánh chưa merge không bao giờ bị xoá; worktree của nó chỉ bị dọn khi sạch và (quá hạn ngày hoặc ổ đĩa gần đầy).
3. Nhánh làm gốc của task chưa xong, nhánh đang checkout, nhánh không có trong DB: không bị xoá.
4. `enabled = false` hoặc cấu hình sai khoảng: không dọn / bị từ chối rõ ràng; mọi test cũ pass.
5. Chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): housekeeping — auto cleanup of finished task worktrees and merged branches` (mục 1-5), `docs: housekeeping config and cleanup --branches` (mục 6). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify (số pass/fail), phần chưa kiểm (chưa chạy với dự án thật, serve chưa restart).
