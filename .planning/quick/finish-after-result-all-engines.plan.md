# Plan: run đã báo kết quả cuối thì không chờ vô hạn tiến trình thoát — áp dụng cho mọi engine, không chỉ agy

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (task 59, 2026-11-12 17:45-17:57)
Run Dev 312 của task 59 (engine `claude`, `claude -p`) đã báo xong: `stream.jsonl` có sự kiện `result` (`stop_reason: end_turn`, chi phí $1,08), commit `44d65095` đã tạo. Tiến trình `claude` (PID 1621891) vẫn sống, không có tiến trình con, không ghi thêm dòng nào. agent-brain chờ nó thoát: 8 phút sau run vẫn `running` trong DB, autopilot không chuyển sang code-review và QA, slot của dự án bị giữ, và các việc khác (merge, restart serve) phải chờ. Phải `agent-brain kill 59` bằng tay (run bị đánh `killed`) rồi `auto 59`.

Nguyên nhân (đã đọc code): `RunRequest.finish_after_result` ("sau sự kiện `result` cuối, tiến trình có từng này giây để tự thoát, sau đó dừng cả cây tiến trình và tính là run kết thúc bình thường", `RunStop::AfterResult`, `runner.rs` ~dòng 633-665, ánh xạ trạng thái ~dòng 835+) chỉ được bật khi engine là agy:
- `agent-brain/src/roles/mod.rs` ~dòng 788: `finish_after_result: (ep.stream == StreamFormat::Agy).then(|| Duration::from_secs(AGY_AFTER_RESULT_GRACE_SECS))`, hằng `AGY_AFTER_RESULT_GRACE_SECS = 15` (~dòng 64);
- `agent-brain/src/orchestrator/triage.rs` ~dòng 694: `agy.then(...)` cùng hằng.
Với engine `claude` (và các engine khác) giá trị là `None`, nên runner chỉ chờ tiến trình thoát hoặc hạn chót tổng của run. Bộ nhận biết kết quả cuối (`stream.rs::result_seen`, qua `RunObserver::result_seen`) đã dùng chung cho các định dạng stream. Đây là lần thứ hai gặp "đã xong nhưng không thoát" (lần trước: `run-task.mjs` của task 55). Cơ chế `run_stall_minutes` đã có trong `main` (đã merge, chưa cài vào serve) chỉ bắt run im lặng sau 20 phút và đánh `timed_out`, không phải kết quả thành công.

## Luật mới
1. Mọi engine có luồng sự kiện dạng stream đều dùng `finish_after_result`: agy giữ 15 giây; các engine khác (claude, và mọi định dạng khác mà `result_seen` nhận biết) 30 giây. Khai báo hằng riêng cho ngưỡng không-agy (ví dụ `AFTER_RESULT_GRACE_SECS = 30`), giữ nguyên hằng agy.
2. Trạng thái cuối khi `RunStop::AfterResult`: giữ đúng ánh xạ hiện có (run kết thúc bình thường, mã thoát thật được ghi lại, trừ khi có yêu cầu kill từ nơi khác thì là `Killed`). KHÔNG được biến một run mà sự kiện `result` báo LỖI (ví dụ `is_error: true` hoặc `subtype` lỗi như `error_max_turns`, `error_during_execution`) thành `Succeeded`: nếu ánh xạ hiện tại có thể làm vậy với claude, sửa để lấy trạng thái từ kết quả đã phân tích của sự kiện `result` (dùng thông tin `stream.rs` đã có, không phân tích lại JSON bằng regex).
3. Thông báo khi dừng không còn nói riêng "agy": `Run đã trả kết quả nhưng tiến trình chưa thoát sau {N}s — dừng cây tiến trình` (một câu cho mọi engine).
4. Không đổi hành vi của agy ngoài câu thông báo.

## Hiện trạng (đã đọc code)
- `agent-brain/src/runner.rs`: `RunRequest.finish_after_result` (~dòng 219), vòng `select!` (nhánh `after_result` ~dòng 633-665, thông báo "agy đã trả kết quả nhưng chưa thoát sau …"), `enum RunStop::AfterResult` (~dòng 292), ánh xạ `Some(RunStop::AfterResult) => …` (~dòng 835+).
- `agent-brain/src/roles/mod.rs`: dựng `RunRequest` cho role (~dòng 777-795); `RunObserver::result_seen` (~dòng 1025) → `self.summary.result_seen()`.
- `agent-brain/src/stream.rs`: `result_seen` (~dòng 311), `feed` (~dòng 315+) đặt cờ khi gặp sự kiện `result`.
- `agent-brain/src/orchestrator/triage.rs`: dựng `RunRequest` cho phân loại (~dòng 686-704), `TriageObserver`.
- Test đã có cho `AfterResult` của agy: tìm bằng `grep -rn "AfterResult\|finish_after_result" tests/ src/ --include=*.rs`.

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/roles/mod.rs` | hằng ~dòng 64; dựng `RunRequest` ~dòng 788 | Thêm hằng cho engine không-agy (30 giây) cạnh `AGY_AFTER_RESULT_GRACE_SECS`; `finish_after_result: Some(Duration::from_secs(<agy 15 hoặc khác 30>))` theo `ep.stream`. Kiểm `RunObserver::result_seen` của role có dùng được cho claude (đọc `stream.rs::feed`; nếu cờ chỉ đặt cho định dạng agy thì sửa để đặt cho cả định dạng claude). | Mọi engine có hạn thoát |
| 2 | `agent-brain/src/orchestrator/triage.rs` | ~dòng 686-704 và `TriageObserver::result_seen` | Cùng luật: bỏ điều kiện `agy.then`, dùng hằng theo loại engine; bảo đảm `TriageObserver` trả `result_seen` đúng cho claude. | Phân loại cũng có thể kẹt |
| 3 | `agent-brain/src/runner.rs` | nhánh `after_result` của `select!`; ánh xạ `AfterResult` → `RunStatus` | Đổi câu thông báo theo luật 3. Xem ánh xạ `AfterResult` (~dòng 835+): bảo đảm luật 2 (kết quả `result` báo lỗi không thành `Succeeded`). Nếu cần truyền tín hiệu "result báo lỗi", thêm một phương thức mặc định `fn result_is_error(&self) -> bool { false }` vào `RunObserver` và triển khai cho observer của role/triage từ `stream.rs`. | Không che lỗi |
| 4 | `agent-brain/tests/run_stall.rs` (đã có từ plan trước) hoặc test hiện có cho `AfterResult`; nếu không hợp thì `agent-brain/tests/run_after_result.rs` mới | test | Tiến trình giả (script python3 trong thư mục tạm; KHÔNG claude hay agy thật; chờ bằng `python3 -c 'import time; time.sleep(..)'` trong script giả) in vài dòng stream-json của claude, rồi dòng `{"type":"result",...}`, rồi không thoát. Ca kiểm với `finish_after_result` vài giây: (a) kết quả thành công → run kết thúc `Succeeded` trong thời gian grace, cây tiến trình (kể cả tiến trình con) bị dừng, không còn tiến trình mồ côi; (b) sự kiện `result` báo lỗi (`is_error: true`) → KHÔNG thành `Succeeded`; (c) tiến trình in `result` rồi tự thoát trước hạn → kết thúc bình thường, không dừng; (d) không có sự kiện `result` và im lặng → không bị dừng bởi cơ chế này (chỉ bởi `Timeout`/`stall_after`); (e) cho agy vẫn đúng như trước (test agy hiện có vẫn pass). Thời gian test ngắn (grace ≤ 3 giây). | Chốt hành vi |
| 5 | `agent-brain/README.md` | mục cấu hình/chạy | Một dòng: mọi engine được dừng sau khi đã trả kết quả nhưng không thoát (agy 15 giây, engine khác 30 giây). | Tài liệu |

## Không được đụng tới
- `autopilot/*`, `claim.rs`, `orchestrator/scheduler.rs`, `db.rs`, `dashboard/*`, `telegram/*`, `config.rs`, `guard.rs`, `workspace.rs`, `gitops.rs`.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db, runs, worktrees thật; serve và task đang chạy: test dùng thư mục tạm), `/mnt/data/source_codes/**`, `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- KHÔNG thay binary `agent-brain` đang chạy. KHÔNG dùng `pkill`/`killall`/kill theo tên tiến trình (CLAUDE.md của repo); chỉ dừng tiến trình do bạn chạy bằng PID bạn đã ghi. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.
- Ổ đĩa `/mnt/data` từng đầy: đặt `CARGO_TARGET_DIR=/mnt/data/IDE/QuangVy-Orchestration/agent-brain/target` cho mọi lệnh cargo (dùng chung thư mục build của main, KHÔNG tạo `target/` riêng trong worktree). Không xoá thư mục đó.

## Cách kiểm tra
Trong worktree của bạn, thư mục `agent-brain/`, với `export CARGO_TARGET_DIR=/mnt/data/IDE/QuangVy-Orchestration/agent-brain/target`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --test run_stall --test runner_kill 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test run_after_result 2>&1 | grep -E "^test result|FAILED|panicked"   # hoặc file test bạn đã chọn ở mục 4
cargo test --offline --locked --test orchestrator_triage 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(runner|roles/mod|orchestrator/triage|stream)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test chập chờn đã biết (không phải của việc này; chạy lại riêng và ghi vào summary): `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. Engine `claude` đã báo `result` thành công mà tiến trình không thoát thì bị dừng sau 30 giây, run kết thúc `Succeeded`, cây tiến trình dừng hết.
2. Sự kiện `result` báo lỗi không bao giờ thành `Succeeded` nhờ cơ chế này.
3. agy giữ 15 giây và hành vi cũ; mọi test cũ pass.
4. Chỉ các file trong bảng Thay đổi bị sửa (cộng phương thức mặc định trong `RunObserver` nếu cần).

## Quy trình
- Hai commit: `fix(agent-brain): stop any engine that already returned its result but does not exit` (mục 1, 2, 3, 4), `docs: finish-after-result applies to every engine` (mục 5). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là cách xử lý result lỗi, cách `result_seen` hoạt động cho claude), kết quả từng lệnh verify (số pass/fail), phần chưa kiểm (chưa chạy với claude/agy thật, serve chưa restart).
