# Plan: dừng run bị treo (không có hoạt động nào quá N phút) và sửa test chập chờn `running_run_adds_kill_button`

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (task 55, 2026-11-12)
Run Dev 269 của task 55 treo 23+ phút mà hệ thống không biết:
- 11:32:43: agy gọi `run_command` (chạy `node .gsd/scripts/run-task.mjs …`). Bộ test xong lúc 11:33:19, nhưng wrapper `run-task.mjs` rơi vào vòng lặp vô hạn nên lệnh không bao giờ trả về.
- `stream.jsonl` không có sự kiện nào nữa; bước 42 ở trạng thái `ACTIVE` mãi. `stderr.txt` chỉ ghi "root agent idle; waiting up to 1h59m30s for 1 background task(s)".
- agent-brain chỉ có hạn chót tổng của run (`RunRequest.timeout`, ở đây 2 giờ: file `deadline` = 13:29). Run giữ chỗ duy nhất của dự án (`max_parallel_tasks = 1`) nên các task khác cũng bị chặn. Người dùng phải tự `agent-brain kill 55`.
- `tool_timeout_seconds` của engine (`config.rs` ~dòng 275) chỉ để nói cho prompt biết giới hạn lệnh shell, KHÔNG cắt lệnh nào.
- Process tree không phân biệt được "đang chạy lâu" với "đang treo": tiến trình treo vẫn ngốn CPU 83%. Vì vậy tín hiệu duy nhất đủ tin cậy là thời gian im lặng của luồng sự kiện.

Ngoài ra khi chạy test, `running_run_adds_kill_button` (`tests/telegram_actions.rs:1714`) fail khi chạy riêng, và cũng fail trên `main` chưa sửa (đã xác nhận bởi hai executor trước), nhưng pass trong lần chạy toàn bộ.

## Hiện trạng (đã đọc code)
- `agent-brain/src/runner.rs`: `RunRequest` (~dòng 197-225: `timeout`, `kill_grace`, `finish_after_result`, `env_watch`…); vòng `tokio::select!` chính (~dòng 559-700) xử lý: `env_rx` (`RunStop::EnvDown`), `rx.recv()` (mỗi dòng sự kiện của engine, gọi `obs.on_line`), `child.wait()`, tín hiệu (`RunStop::Signal`), hết giờ (`RunStop::Timeout`, `spawn_stop`); `enum RunStop` (~dòng 275: `Signal`, `Timeout`, `AfterResult`, `Observer`, `EnvDown`); ánh xạ `RunStop` → `RunStatus` (~dòng 795-810: `Timeout` → `TimedOut`).
- `agent-brain/src/roles/mod.rs` ~dòng 777: nơi dựng `RunRequest` cho role (cũng `orchestrator/triage.rs` ~dòng 683 cho triage).
- `agent-brain/src/config.rs`: các khoá cấu hình có kiểm khoảng giá trị (mẫu: `merge.git_timeout_secs` ~dòng 94-99, `tool_timeout_seconds` ~dòng 657-675).

## Luật mới
1. Mỗi run có `stall_after: Option<Duration>`. Nếu trong khoảng này không có DÒNG NÀO từ stdout/stream của engine (mọi dòng đều tính là hoạt động), run bị dừng bằng cùng đường dừng của `Timeout` (`spawn_stop`, tôn trọng `kill_grace`, container stop), với `RunStop::Stalled`.
2. Trạng thái kết thúc: `RunStatus::TimedOut` (để autopilot, dashboard, Telegram xử lý như hết giờ, không thêm trạng thái mới trong DB). `runs.error` = `Run không có hoạt động nào trong {N} phút (có thể một lệnh bị treo) — đã dừng`. `obs.on_notice` in cùng câu.
3. Mặc định `stall_after` = 20 phút cho mọi engine. Cấu hình `run_stall_minutes` ở cấp trên cùng của `config.json`: số nguyên 5 đến 240, `0` = tắt; ngoài khoảng → lỗi cấu hình có thông báo tiếng Việt (như các khoá khác). Thiếu khoá → 20.
4. Đồng hồ im lặng bắt đầu tính từ khi run khởi chạy (khung khởi động của agy cũng phát sự kiện `init`; nếu một engine không phát gì trong 20 phút đầu thì đúng là treo). Không đặt lại đồng hồ bằng stderr.
5. Không áp dụng khi `stopping` đã có giá trị (đang dừng), và không áp dụng cho các lần chạy của người dùng ở chế độ tương tác nếu có (xem `roles/mod.rs`: nếu có nhánh `human`/engine người thì `stall_after = None`).
6. Hiển thị: dùng đường hiện có cho `timed_out` (autopilot dừng "hết giờ"). Không thêm chữ mới ở dashboard/Telegram trừ khi chúng đang in lý do từ `runs.error` (khi đó câu lý do ở luật 2 sẽ hiện).

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/config.rs` | cấu hình cấp trên cùng, hàm đọc/kiểm khoảng | Thêm trường (ví dụ `run_stall_minutes: u64`, mặc định 20) vào struct cấu hình chính, đọc từ `config.json` khoá `run_stall_minutes`, kiểm 0 hoặc 5..=240, lỗi tiếng Việt khi sai; thêm vào danh sách khoá được phép nếu có kiểm khoá lạ. Test: mặc định 20, giá trị hợp lệ, `0` tắt, ngoài khoảng bị từ chối. | Cấu hình |
| 2 | `agent-brain/src/runner.rs` | `RunRequest`, `RunStop`, vòng `select!`, ánh xạ trạng thái | Thêm `stall_after: Option<Duration>` vào `RunRequest`; thêm `RunStop::Stalled`; giữ `last_activity: Instant` cập nhật mỗi khi `rx.recv()` trả một dòng; thêm nhánh `select!` dùng `tokio::time::sleep_until(last_activity + stall_after)` (đặt lại sau mỗi dòng; vô hiệu khi `stopping.is_some()` hoặc `stall_after` là `None`) → `stopping = Some(RunStop::Stalled)`, `obs.on_notice(…)`, `spawn_stop(...)` như nhánh `Timeout`; `RunStop::Stalled` → `RunStatus::TimedOut`; lưu lý do dừng vào chỗ kết quả đang mang `EnvDown` (`env_down`/trường tương tự) hoặc thêm trường `stalled_minutes` để lớp trên ghi `error`. Mọi `match RunStop` khác phải được cập nhật cho biên dịch. | Dừng run treo |
| 3 | `agent-brain/src/roles/mod.rs` và `agent-brain/src/orchestrator/triage.rs` | nơi dựng `RunRequest` (~dòng 777 và ~683) | Điền `stall_after` từ cấu hình (`None` khi `run_stall_minutes = 0`); ghi `runs.error` theo luật 2 khi kết quả là `Stalled` (đi cùng cách các lỗi `TimedOut` hiện được ghi). Mọi nơi khác dựng `RunRequest` (kể cả test) dùng `stall_after: None` hoặc mặc định hợp lý. | Nối cấu hình vào run |
| 4 | `agent-brain/tests/` — thêm test mới trong file đã có test runner/`Timeout` (tìm bằng `grep -rn "Timeout\|timed_out\|fake" tests/ | head`; nếu chưa có thì `tests/run_stall.rs`) | cả test | Dùng tiến trình giả (script `sh`/`python3` trong thư mục tạm, KHÔNG agy thật, KHÔNG dùng `sleep` ở foreground của shell tự do: dùng `python3 -c 'import time; time.sleep(..)'` trong script giả). Ca kiểm: (a) engine in vài dòng rồi im lặng lâu hơn `stall_after` (đặt vài giây cho test) → run kết thúc `TimedOut`, `error` chứa "không có hoạt động", toàn bộ cây tiến trình bị dừng (không còn tiến trình mồ côi); (b) engine in dòng đều đặn dưới ngưỡng → KHÔNG bị dừng, kết thúc bình thường; (c) `stall_after = None` → không bị dừng dù im lặng (kết thúc bằng thoát bình thường); (d) im lặng nhưng đã có `Signal` đang dừng → không đặt lại trạng thái. Thời gian test ngắn (ngưỡng ≤ 3 giây). | Chốt hành vi |
| 5 | `agent-brain/tests/telegram_actions.rs` | `running_run_adds_kill_button` (~dòng 1700-1720) | Tìm nguyên nhân thật vì sao assert ở dòng 1714 fail khi chạy riêng (đọc test và `tests/support/telegram.rs`: đoán là chờ không đủ lâu, thứ tự tin nhắn, hoặc dữ liệu chưa sẵn lúc kiểm). Nếu là lỗi test (race, chờ cố định): sửa test dùng chờ theo điều kiện có hạn (thăm dò đến khi đúng hoặc hết thời gian), KHÔNG tăng `sleep` tuỳ tiện. Nếu là lỗi thật của code (`telegram/render.rs`/`detect.rs`): DỪNG, không sửa code, ghi chi tiết vào summary. Test phải pass 5/5 lần khi chạy riêng và khi chạy cả binary. | Test ổn định |
| 6 | `agent-brain/README.md` | mục cấu hình | Mô tả `run_stall_minutes` (mặc định 20, 5-240, 0 = tắt), cách hiểu "không có dòng nào từ engine", và hậu quả: run bị đánh `timed_out`. | Tài liệu |

## Không được đụng tới
- Logic autopilot (`autopilot/*`), `claim.rs`, hàng chờ (`orchestrator/scheduler.rs`, `db.rs`: plan khác lo), `dashboard/*`, `telegram/*` (trừ test ở mục 5), `guard.rs`, `workspace.rs`, `gitops.rs`.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db thật, run thật, worktree thật; serve đang chạy: test dùng thư mục tạm), `/mnt/data/source_codes/**`, `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- KHÔNG thay binary `agent-brain` đang chạy. KHÔNG dùng `pkill`/`killall` hay kill theo tên tiến trình (CLAUDE.md của repo): dừng tiến trình test của chính bạn bằng PID bạn đã ghi. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --lib config runner 2>&1 | grep -E "^test result|FAILED|panicked"      # nếu cargo nhận một bộ lọc mỗi lần, chạy từng bộ lọc
cargo test --offline --locked --test run_stall 2>&1 | grep -E "^test result|FAILED|panicked"          # hoặc file test bạn đã đặt
for i in 1 2 3 4 5; do cargo test --offline --locked --test telegram_actions running_run_adds_kill_button 2>&1 | grep -E "^test result|FAILED|panicked"; done
cargo test --offline --locked --test telegram_actions 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(runner|config|roles/mod|orchestrator/triage)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test chập chờn đã biết khác (không phải của việc này; chạy lại riêng và ghi vào summary): `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. Run không có dòng nào từ engine quá `run_stall_minutes` phút bị dừng (cả cây tiến trình), kết thúc `timed_out` với lý do rõ trong `runs.error`; run đều đặn có sự kiện không bị dừng.
2. `run_stall_minutes = 0` tắt; thiếu khoá = 20; giá trị sai bị từ chối.
3. Các test cũ pass; `running_run_adds_kill_button` pass 5/5 lần chạy riêng.
4. Chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Ba commit: `feat(agent-brain): stop runs that produce no engine output for run_stall_minutes` (mục 1, 2, 3, 4), `test(agent-brain): make running_run_adds_kill_button stable` (mục 5), `docs: run_stall_minutes` (mục 6). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là cách giữ lý do `Stalled` và nguyên nhân thật của test chập chờn), kết quả từng lệnh verify (số pass/fail), phần chưa kiểm (chưa chạy với agy thật, serve chưa restart).
