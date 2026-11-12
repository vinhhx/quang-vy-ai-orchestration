# Plan: `agent-brain doctor` — job theo dõi run/task bị treo và cảnh báo qua Telegram

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (2026-11-12)
Hôm nay gặp ba kiểu "chờ vô vọng", cả ba chỉ được phát hiện khi người dùng hỏi:
- Task 55: bộ test xong nhưng `run-task.mjs` lặp vô hạn, run Dev treo 23+ phút.
- Task 59: `claude -p` đã báo `result` nhưng không thoát, run `running` 8+ phút, slot dự án bị giữ.
- Ổ đĩa `/mnt/data` đầy 99%.
Đã có hai lớp trong tiến trình giám sát run (đã merge): `finish_after_result` (mọi engine) và `run_stall_minutes`. Cả hai nằm TRONG tiến trình giám sát; nếu chính nó treo hay chết, hoặc có việc khác (driver autopilot mồ côi, ổ đĩa đầy, serve không chạy), không ai biết. Cần lớp thứ ba, độc lập: một bộ quét định kỳ, chỉ cảnh báo, và chỉ tự sửa những ca chắc chắn.

## Hiện trạng (đã đọc code)
- `agent-brain/src/runner.rs`: `reconcile_stale_runs(conn)` (~dòng 32-52) đánh `lost` mọi run `running` có pid không còn sống hoặc thiếu pid; `crate::procs::alive(ProcId)` (procs.rs ~dòng 48). `db::mark_lost_if_running(conn, run_id)` (db.rs ~dòng 749), `db::running_runs(conn)`, `db::auto_driver_rows(conn)`, `autopilot::claim::{driver_alive, row_alive, run_alive}`.
- `agent-brain/src/telegram/detect.rs`: mẫu bộ quét `IntakeFailures`, `EnvReruns`, `ChainWaits`, `JournalMail`: mỗi cái có `seed(conn)`/`scan(conn) -> BrainResult<Vec<Outgoing>>`, được khởi tạo và gọi trong `run()` (mỗi tin hiệu DB, và quét đầy đủ mỗi 60 giây `SAFETY_SCAN_SECS`); `Outgoing::Message { text }` gửi tin không nút (`telegram/outbox.rs`).
- `agent-brain/src/telegram/setup.rs` (~dòng 303, 375): đã có đường gửi `sendMessage` từ CLI (lệnh `telegram` gửi tin thử) — dùng lại cho `doctor --notify`.
- `agent-brain/src/config.rs`: `struct Config` (có `run_stall_minutes`); `Config::stall_after()`.
- Mỗi run có thư mục `runs.run_dir` với `stream.jsonl` (mtime = lần cuối engine ghi) và `deadline`.
- Bảng `queued_actions` (migration 13), `tasks.auto_state`, `tasks.updated_at`.

## Luật mới
1. Mô-đun `agent-brain/src/doctor.rs` với hàm thuần theo tham số `scan(conn, now_ms, opts: &DoctorOpts) -> BrainResult<Vec<Finding>>`; `Finding { kind, key, task_id, run_id, text }` (`key` ổn định để chống gửi lặp, ví dụ `"run-silent:312"`). Các loại phát hiện (mỗi loại một câu `text` tiếng Việt, có gợi ý lệnh):
   a. `run-dead`: run `running` mà tiến trình đã chết (pid không còn sống), HOẶC thiếu pid nhưng `started_at` cũ hơn 2 phút (tránh chạy đua lúc vừa spawn). CÓ THỂ tự sửa: `db::mark_lost_if_running`.
   b. `result-no-exit`: run `running`, tiến trình còn sống, `stream.jsonl` có dòng sự kiện `result` cuối (đọc 16 KiB cuối của tệp, tìm dòng JSON có `"type":"result"` hoặc `"event":"result"`) và mtime của tệp cũ hơn `result_no_exit_minutes` (mặc định 3). Chỉ cảnh báo (lớp trong tiến trình lẽ ra đã xử lý).
   c. `run-silent`: run `running`, sống, mtime `stream.jsonl` cũ hơn `run_stall_minutes + 5` phút (nếu `run_stall_minutes = 0` thì dùng `silent_minutes` mặc định 30). Chỉ cảnh báo, gợi ý `agent-brain kill <task>`.
   d. `run-long`: run `running` quá `run_max_minutes` (mặc định 150). Chỉ cảnh báo.
   e. `driver-stale`: task có `auto_state` là `running` hoặc `stopping` nhưng không có driver sống (`driver_alive`) và không có run sống của task đó, và `updated_at` cũ hơn `driver_stale_minutes` (mặc định 10). Chỉ cảnh báo, gợi ý `agent-brain auto <id>` hoặc `kill`.
   f. `queue-stuck`: hàng `queued_actions` ở `waiting` quá `queue_stuck_minutes` (mặc định 60) trong khi dự án không bận. Chỉ cảnh báo.
   g. `disk-low`: dung lượng trống của hệ tệp chứa thư mục dữ liệu và thư mục worktree (lấy bằng `df -Pk`, chỉ thư viện chuẩn) nhỏ hơn `housekeeping.min_free_gb` (nếu khoá này chưa có trong `Config` vì executor khác đang thêm, dùng `doctor.disk_min_gb` mặc định 20). Chỉ cảnh báo.
   h. `serve-down` (chỉ khi gọi với `--check-serve`): tệp `serve.pid` trong thư mục dữ liệu trỏ tới tiến trình không còn sống trong khi DB còn run `running` hoặc task `auto_state = running`. Chỉ cảnh báo.
2. Cấu hình: khối `doctor` ở cấp trên cùng của `config.json`: `{ "enabled": true, "result_no_exit_minutes": 3, "silent_minutes": 30, "run_max_minutes": 150, "driver_stale_minutes": 10, "queue_stuck_minutes": 60, "disk_min_gb": 20, "realert_minutes": 60 }`. Thiếu → mặc định như trên. Mỗi số phải nằm trong khoảng hợp lý (ví dụ phút 1..=1440, `disk_min_gb` 0..=2000, 0 = tắt cảnh báo ổ đĩa); sai → lỗi cấu hình tiếng Việt như các khoá khác. Đặt trường mới NGAY SAU `run_stall_minutes` trong `Config` (một executor khác thêm khối `housekeeping` ở cuối).
3. Chống gửi lặp: một phát hiện (`key`) chỉ được gửi lần đầu và gửi lại sau `realert_minutes`; hết phát hiện thì quên `key` (lần sau xuất hiện lại là mới). Trạng thái: trong serve giữ trong bộ nhớ; trong CLI giữ ở `~/.local/share/agent-brain/doctor-state.json` (mode 600; đường dẫn lấy từ `config::data_dir()`; test dùng thư mục tạm).
4. Trong serve: thêm bộ quét `DoctorAlerts` vào `telegram/detect.rs` theo mẫu `IntakeFailures` (khởi tạo cùng chỗ, gọi trong mảng `results` mỗi lượt quét đầy đủ ≥ 60 giây; KHÔNG gọi mỗi 500 ms): mỗi lần quét chạy `doctor::scan`, tự sửa phát hiện `run-dead`, rồi trả `Outgoing::Message` cho các phát hiện mới hoặc đến hạn gửi lại, mỗi tin một dòng tóm tắt (tối đa 5 phát hiện mỗi tin, phần còn lại `… và N cảnh báo nữa`); đồng thời `term::emit` cùng nội dung. Tin không chứa đường dẫn tuyệt đối, token, hay nội dung yêu cầu của người dùng (chỉ id task/run, tên dự án, tên quy trình).
5. CLI: `agent-brain doctor [--json] [--notify] [--heal] [--check-serve]`. Mặc định chỉ in danh sách phát hiện (mỗi dòng một câu), mã thoát 0 khi không có, 1 khi có. `--heal` áp dụng tự sửa `run-dead`. `--notify` gửi một tin Telegram tóm tắt các phát hiện mới theo luật 3 (dùng đường gửi của `telegram/setup.rs`; cần cấu hình Telegram, thiếu thì in thông báo và mã thoát 2). `--json` in mảng JSON `{kind,key,task_id,run_id,text}`.
6. Chỉ tự sửa `run-dead`. KHÔNG bao giờ tự kill run, tự đổi `auto_state`, tự xoá file. Mọi phát hiện khác chỉ báo.
7. `enabled = false` tắt bộ quét trong serve; CLI `doctor` vẫn chạy khi gọi tay.

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/config.rs` | `struct Config`, đọc cấu hình | Thêm `Doctor { enabled, result_no_exit_minutes, silent_minutes, run_max_minutes, driver_stale_minutes, queue_stuck_minutes, disk_min_gb, realert_minutes }`, trường `Config.doctor` NGAY SAU `run_stall_minutes`, kiểm khoảng (luật 2), thêm khoá `doctor` vào danh sách khoá cấp trên cùng được phép nếu có. Test: mặc định, hợp lệ, ngoài khoảng, sai kiểu. | Cấu hình |
| 2 | `agent-brain/src/doctor.rs` (mới) và `agent-brain/src/lib.rs` (một dòng `pub mod doctor;`) | `scan`, `heal_dead_runs`, `format_finding`, `Dedup` (trạng thái chống lặp, tuần tự hoá ra JSON), `free_gb_of(dir)` | Luật 1, 3, 6. Đọc `stream.jsonl` bằng cách đọc đuôi tệp (không đọc cả tệp), mtime qua `std::fs::metadata`. Tái dùng `procs::alive`, `driver_alive`, `db::mark_lost_if_running`. Hàm nhận mọi ngưỡng qua `DoctorOpts` và nhận `now_ms`, `free_gb: Option<u64>` để test. | Logic phát hiện |
| 3 | `agent-brain/src/telegram/detect.rs` | struct `DoctorAlerts`, `run()` | Luật 4. Khởi tạo cùng các bộ quét khác, đưa vào mảng `results`; chỉ chạy khi `full` (quét 60 giây) và `cfg.doctor.enabled`. Test đơn vị cạnh các test của `IntakeFailures`: tin đầu tiên, không lặp, gửi lại sau `realert`, tự quên khi hết phát hiện, giới hạn 5 phát hiện mỗi tin. | Cảnh báo trong serve |
| 4 | `agent-brain/src/main.rs` (và `agent-brain/src/doctor.rs` cho `cmd_doctor`) | lệnh con `doctor` | Luật 5. Mã thoát: 0 sạch, 1 có phát hiện, 2 lỗi (thiếu cấu hình Telegram khi `--notify`, không đọc được DB). | CLI cho cron |
| 5 | `agent-brain/tests/doctor.rs` (mới) | cả file | Thư mục tạm, DB tạm, `XDG_DATA_HOME` về thư mục tạm. Tiến trình giả bằng `python3` do test tự chạy (ghi PID, dừng đúng PID; không dùng `pkill`). Ca kiểm: (a) run `running` với pid đã chết → `run-dead`; `--heal`/`heal_dead_runs` đánh `lost`; run mới spawn thiếu pid chưa quá 2 phút → không báo; (b) run sống, `stream.jsonl` có dòng `result` và mtime cũ → `result-no-exit`; mtime mới → không báo; (c) run sống, `stream.jsonl` im quá ngưỡng → `run-silent`; ghi gần đây → không; (d) run quá `run_max_minutes` → `run-long`; (e) task `auto_state=running` không driver, không run, `updated_at` cũ → `driver-stale`; có driver sống → không; (f) hàng `queued_actions` waiting quá ngưỡng, dự án rảnh → `queue-stuck`; (g) `free_gb` dưới ngưỡng → `disk-low`; `disk_min_gb=0` → không; (h) `--check-serve`: `serve.pid` chết mà còn run `running` → `serve-down`; (i) chống lặp: gửi lần đầu, không gửi lại trong `realert`, gửi lại sau `realert`, quên khi hết phát hiện, trạng thái đọc/ghi qua tệp JSON; (j) CLI: `doctor` không phát hiện → mã 0, có → mã 1, `--json` hợp lệ, `--notify` không có cấu hình Telegram → mã 2; (k) tin không chứa đường dẫn tuyệt đối. | Chốt hành vi |
| 6 | `agent-brain/README.md` | mục vận hành | Mô tả `doctor`: các loại phát hiện, cấu hình, mã thoát, cách chạy định kỳ bằng cron (ví dụ dòng `*/5 * * * * agent-brain doctor --notify --heal --check-serve >> ~/.local/share/agent-brain/doctor.log 2>&1`), lưu ý "chỉ tự sửa run-dead; mọi ca khác chỉ cảnh báo". KHÔNG tự cài crontab. | Tài liệu |

## Không được đụng tới
- `runner.rs` (chỉ gọi `reconcile_stale_runs` nếu hữu ích, không sửa), `roles/*`, `autopilot/*`, `claim.rs` (chỉ gọi hàm có sẵn), `orchestrator/*`, `dashboard/*`, `workspace.rs`, `db.rs` (chỉ đọc; nếu cần truy vấn mới thì thêm hàm nhỏ và ghi vào summary), phần `telegram/*` khác ngoài `detect.rs`.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db, runs, worktrees thật; serve đang chạy: test dùng thư mục tạm), `/mnt/data/source_codes/**`, `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác. KHÔNG sửa crontab của người dùng.
- KHÔNG thay binary `agent-brain` đang chạy. KHÔNG dùng `pkill`/`killall`/kill theo tên tiến trình (CLAUDE.md của repo); chỉ dừng tiến trình do bạn chạy bằng PID bạn đã ghi. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.
- Ổ đĩa `/mnt/data` từng đầy: với MỌI lệnh cargo đặt `export CARGO_TARGET_DIR=/mnt/data/IDE/QuangVy-Orchestration/agent-brain/target` (thư mục build chung của main; không tạo `target/` trong worktree, không xoá thư mục đó). Một executor khác chạy song song và cũng dùng thư mục này: cargo tự khoá, chỉ có thể phải chờ. Dùng tên tệp riêng cho đầu ra tạm.

## Cách kiểm tra
Trong worktree của bạn, thư mục `agent-brain/`, với `CARGO_TARGET_DIR` như trên:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --lib config 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --lib detect 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test doctor 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test telegram_notify --test telegram_outage --test telegram_serve 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(doctor|telegram/detect|config)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test chập chờn đã biết (chạy lại riêng và ghi vào summary nếu là test duy nhất fail): `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. `agent-brain doctor` liệt kê đúng các phát hiện a-h trên dữ liệu giả; mã thoát 0/1/2 đúng; `--json` hợp lệ.
2. Trong serve, phát hiện mới tạo đúng một tin Telegram, không gửi lặp trong `realert_minutes`; tin không chứa đường dẫn, token hay nội dung yêu cầu.
3. Chỉ `run-dead` được tự sửa; không có đường nào tự kill, đổi `auto_state` hay xoá file.
4. `enabled = false` tắt bộ quét trong serve; mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): doctor — detect hung runs, stale drivers, stuck queue and low disk, alert on Telegram` (mục 1-5), `docs: doctor command and cron example` (mục 6). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify (số pass/fail), phần chưa kiểm (chưa chạy với run thật, chưa gửi tin Telegram thật, serve chưa restart).
