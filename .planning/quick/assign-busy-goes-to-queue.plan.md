# Plan: màn Giao việc không chặn khi dự án đang bận — đưa việc vào hàng chờ

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (người dùng, 2026-11-12)
Giao nhiều việc cho cùng một dự án thì việc thứ hai bị chặn, màn Giao việc hiện:
`Sẽ tạo nhánh task/test-lai-loi-nay-unauthenticated-apidocjs-api` và
`Dự án automation-test-e2e đang chạy 1 task song song (tối đa 1, max_parallel_tasks): task 53 — chờ một task xong`.
Nút Giao việc bị khoá, việc không được ghi nhận gì. Người dùng muốn việc vào hàng chờ, chờ đến lượt hoặc chờ duyệt.

## Nguyên nhân (đã đọc code)
- `GET /api/preflight` (`dashboard/api_tasks.rs`, `LeadPreflight`, ~dòng 520-565) trả `can_start = false`, `kind = "busy"` khi `busy_reason` có lý do (`autopilot/claim.rs::worktree_busy_reason`, thông điệp "… chờ một task xong").
- `agent-brain/assets/dashboard/js/assign.js`: `get canStart` (~dòng 740) đòi `preflight.can_start`, nên `startDisabled` = true và `submit()` không chạy, kể cả khi `orchestratorEnabled` bật.
- Server ĐÃ có hàng chờ và nó chịu được dự án bận, không cần đổi:
  - `POST /api/intake` (`api_intake.rs::create_intake`): ghi `intake` `queued`; orchestrator chia việc → đề xuất `pending` → người dùng duyệt ở màn Điều phối.
  - `POST /api/tasks` với `direct: true` (`api_tasks.rs` ~dòng 1143-1290): ghi thẳng một đề xuất `approved`, không kiểm bận.
  - `orchestrator/scheduler.rs::schedule_tick`: đề xuất `approved` gặp `StartOutcome::Busy` hoặc `PreflightRefused { kind: "busy" }` thì `continue`, giữ `approved` và thử lại ở lượt sau (không đánh `failed`).
- Cấu hình hiện tại: `orchestrator.enabled = true`, `max_parallel = 3` (toàn cục); dự án có `max_parallel_tasks` riêng (ở đây 1).

## Luật mới
1. Khi `orchestratorEnabled` bật và lý do chặn duy nhất của preflight là `kind === "busy"`: nút Giao việc VẪN bấm được. Việc đi đúng đường hiện có: `/api/intake` (chế độ thường) hoặc `/api/tasks` `direct: true` (chế độ trực tiếp). Server không đổi.
2. Màn hình nói rõ việc sẽ xếp hàng: thay khung đỏ bằng khung vàng (amber, như callout team ở ~dòng 942) với câu `Dự án đang bận — việc sẽ vào hàng chờ và tự chạy khi dự án rảnh.` kèm dòng lý do gốc (`reasonHead`, có số task đang chạy); nhãn nút ở trạng thái chưa gửi vẫn là `Giao việc`; sau khi gửi giữ nhãn hiện có (`Đã vào hàng chờ #N` / `Đã tạo đề xuất #N`).
3. Các lý do chặn khác (`dirty`, `branch_exists`, `base_missing`, `exists`, hay không có `kind`) giữ nguyên: khung đỏ, nút khoá. Xếp hàng không giải quyết được chúng và scheduler sẽ đánh `failed` ở lượt chạy.
4. Khi orchestrator TẮT: giữ nguyên hành vi chặn (hàng chờ không chạy khi `enabled = false`, `schedule_tick` thoát sớm). Khung đỏ thêm gợi ý một dòng: `Bật Điều phối ở Cài đặt để việc tự xếp hàng.` (chỉ khi `kind === "busy"`).
5. `dirty` và kiểm tra khác vẫn do preflight hiện có tính; không đổi `LeadPreflight` hay `claim.rs`.

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/assets/dashboard/js/assign.js` | getters `canStart`, `hasRefusal`, `disabledReason`, mới `queuedBusy` và `busyHint` | Thêm `get isBusyOnly()` = `preflight && !preflight.can_start && preflight.kind === 'busy'`; `get queuedBusy()` = `isBusyOnly && orchestratorEnabled`. `canStart` thay `this.preflight.can_start` bằng `(this.preflight.can_start \|\| this.queuedBusy)`. `hasRefusal` trả `false` khi `queuedBusy` (khung đỏ không hiện). `disabledReason`: khi `queuedBusy` không trả `reasonHead`. Thêm `get busyHint()`: khi `queuedBusy` → câu ở Luật mới 2; khi `isBusyOnly && !orchestratorEnabled` → câu ở Luật mới 4; ngược lại rỗng. `submit()` KHÔNG đổi. | Mở nút trong đúng trường hợp |
| 2 | `agent-brain/assets/dashboard/screens/assign.html` | quanh dòng 212-215 (`planLine`, khung đỏ `hasRefusal`) | Thêm khung amber hiển thị khi `queuedBusy`: một dòng `busyHint` và một dòng `reasonHead`; khung đỏ hiện cũ giữ cho các lý do khác; khi `isBusyOnly && !orchestratorEnabled` hiện thêm `busyHint` trong khung đỏ. Dùng các lớp Tailwind đã có trong `app.css` (đã build sẵn, KHÔNG chạy tailwind, KHÔNG sửa `app.css`): lấy y nguyên bộ lớp của callout amber ở assign.html (tìm callout team, dòng có `amber`); nếu lớp nào không có trong `app.css`, dùng lớp tương đương đã có. CSP: không inline script/style, chỉ Alpine CSP (`x-show`, `x-text`, getter) như cấu trúc hiện có, không biểu thức phức tạp trong thuộc tính. | Hiển thị đúng |
| 3 | `agent-brain/tests/dashboard_assets.rs` | thêm test | Test giữ chuỗi trong asset (không có khung test JS): `assign.js` chứa `isBusyOnly`, `queuedBusy`, `busyHint`, và `canStart` không còn đòi riêng `preflight.can_start`; `screens/assign.html` chứa `busyHint`. Chỉ đọc file asset qua `include_str!` hoặc đường dẫn như test hiện có. | Khoá hành vi, vì không chạy được JS trong test |
| 4 | `agent-brain/tests/` (file đang test hàng chờ / intake qua HTTP: `intake_api.rs` hoặc `dashboard_http.rs`, dùng helper của file đó) | thêm test | Dựng dự án chế độ worktree `max_parallel_tasks = 1` với một run đang sống (helper có sẵn của `dashboard_run.rs`/`autopilot_driver.rs`, tìm cách họ làm "dự án bận"; nếu quá nặng, giả bằng một dòng `runs` `running` với pid của chính tiến trình test). (a) `GET /api/preflight` trả `can_start = false`, `kind = "busy"`; (b) với orchestrator bật, `POST /api/intake` trả 201 và `POST /api/tasks` `direct:true` trả 201 `status: approved`; (c) gọi `orchestrator::scheduler::schedule_tick` khi dự án còn bận: đề xuất vẫn `approved`, không `failed`, chưa có task mới. Nếu (c) đã có test tương đương (`tests/orchestrator_scheduler.rs`), chỉ tham chiếu, không nhân đôi. | Chứng minh server xếp hàng đúng |
| 5 | `agent-brain/README.md` | mục Giao việc / hàng chờ | Một dòng: dự án đang bận thì Giao việc xếp việc vào hàng chờ khi Điều phối bật; lý do khác (working tree bẩn, nhánh đã có, thiếu nhánh gốc) vẫn chặn. | Tài liệu |

## Không được đụng tới
- Phía server: `dashboard/api_tasks.rs`, `autopilot/claim.rs`, `orchestrator/*`, `db.rs`: chỉ ĐỌC (ngoại lệ duy nhất: nếu test mục 4 chứng minh server sai, DỪNG và báo trong summary, không tự sửa).
- `assets/dashboard/app.css` và `vendor/`, mọi asset khác, bảng `ASSETS` trong `assets.rs` (không thêm file mới; nếu thêm thì phải cập nhật bảng và test `assets_table_matches_tree`).
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db thật; serve đang chạy), `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- KHÔNG thay binary `agent-brain` đang chạy. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --test dashboard_assets 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test assets_parity --test dashboard_http --test intake_api --test orchestrator_scheduler 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Một vài test timing (`running_run_adds_kill_button` trong `telegram_actions`, `export_error_is_recorded_once_and_backs_off` trong `journal_safety`) từng flaky trên nền gốc; nếu chúng là test duy nhất fail, chạy lại riêng từng cái và ghi rõ trong summary.

Acceptance:
1. Orchestrator bật + preflight `kind: busy`: nút Giao việc bấm được, khung amber hiện câu xếp hàng và lý do gốc; gửi → việc vào hàng chờ, không lỗi.
2. Lý do khác (`dirty`, `branch_exists`, `base_missing`): vẫn khung đỏ, nút khoá như cũ.
3. Orchestrator tắt + `busy`: nút khoá, có gợi ý bật Điều phối.
4. Server không đổi; scheduler giữ đề xuất `approved` khi bận (test).
5. Chỉ các file trong bảng Thay đổi bị sửa; mọi test cũ pass.
6. Việc kiểm bằng mắt trên dashboard thật là phần CHƯA kiểm của executor; ghi rõ trong summary.

## Quy trình
- Hai commit: `feat(agent-brain): assign screen queues work instead of blocking when the project is busy` (assign.js, assign.html, tests), `docs: queued assign when busy` (README). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify (số test pass/fail), phần chưa kiểm (giao diện thật chưa mở, serve chưa restart).
