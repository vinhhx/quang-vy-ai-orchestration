# Plan 3: sửa các vấn đề agent-brain gặp khi chạy e2e apec (2026-11-08)

Nguồn: `.planning/quick/e2e-run-findings.md` (mục 1–27). Đọc file đó trước. HEAD lúc viết: `7b010c8`.

Đã xong ở plan trước, không làm lại:
- mục 1 (level trước Lead đầu tiên): Plan 1b, commit 36a14eb;
- §5b.1–5 của `task-handoff.plan.md`: Plan 1.

Ngoài phạm vi code: mục 6, 11 (phần điều phối viên), 14, 16. Đây là ghi chú prompt hoặc mẫu của dự án e2e, người dùng tự sửa trong `~/.config/agent-brain/prompts/projects/...` và repo e2e.

## 0. Nguyên tắc bắt buộc: không hard code

1. **Không có tên cụ thể trong code** (trừ fixture test).
   - Không xuất hiện tên dự án, quy trình, đội hình hay engine cụ thể: `apec`, `e2e`, `automation-test-e2e`, `e2e-tu-dong`, `tiet-kiem`, `deepseek`…
   - Hành vi khác nhau giữa các dự án phải đến từ cấu hình.
2. **Mọi tùy chọn mới đều là cấu hình có mặc định giữ nguyên hành vi hiện tại.** Tùy chọn mới nằm ở một trong các chỗ:
   - dự án: `projects_map.json`, đọc qua `src/projects.rs`;
   - quy trình: `workflows.json` và `assets/workflows.default.json`, đọc qua `src/workflows.rs`;
   - cấu hình chung: `config.json` và `assets/config.default.json`, đọc qua `src/config.rs`;
   - engine: phần `engines` của config.

   Mỗi tùy chọn cần:
   - kiểm giới hạn giá trị theo cách sẵn có;
   - endpoint hoặc trường trong dashboard để sửa, theo mẫu `api_projects.rs` (PUT …) và `api_settings.rs` (workflow);
   - test cho trường hợp thiếu tùy chọn: hành vi y như cũ.
3. **Con số lấy từ cấu hình, không viết chữ trong prompt hay code.** Ví dụ: số phút timeout, giới hạn một lệnh shell, số lần thử lại.
4. **Tên file giao thức** (`.plan.md`, `.qa-report.md`, `.plan-review.md`, `.code-review.md`) chỉ được định nghĩa ở **một** module. Mọi chỗ dùng phải gọi module đó (xem phase D).
5. **Văn bản hướng dẫn theo loại engine** nằm trong file asset (`assets/prompts/engine-notes/<kind>.md`), không nằm trong chuỗi Rust.
6. **Tái sử dụng cơ chế sẵn có.** Không viết bản thứ hai của các hàm sau:
   - `engine::readiness`;
   - `teams::team_profile_refusal`;
   - `db::merge_run_metrics`;
   - `StageRun.note`;
   - `handle_quota_stops`;
   - `continuation_reason` (dev).

## Cách chạy plan

Có 4 phase. Mỗi phase là **một lần agy riêng** và có commit riêng. Claude review xong phase trước mới giao phase sau.
- Thứ tự A → B → C → D.
- Phase D lớn nhất, có thể tách tiếp nếu review A–C thấy cần.

Chỉ sửa trong:
- `agent-brain/src/**`
- `agent-brain/tests/**`
- `agent-brain/assets/**`

## Phase A: lý do lỗi phải nhìn thấy được, và các luật autopilot sai

### A1. Lưu lý do thất bại của run (mục 2, 20, 22)

Hiện tại `runs.error` chỉ được ghi trong `db::finish_run` (gọi tại `roles/mod.rs:952-990`), và chỉ cho 2 trường hợp: vi phạm bước tool của agy, và quota. Mọi kiểm tra cấp vai chạy **sau** `finish_run` nên đều để NULL.

1. **Thêm hàm ghi lỗi.** Thêm `db::set_run_error(conn, run_id, &str)` vào `src/db.rs`:
   - lệnh `UPDATE runs SET error = ?`;
   - nội dung đã qua redact giống `roles/mod.rs:981`, cắt còn 4000 byte.

   Thêm một hàm helper trong `src/roles/mod.rs`, ví dụ `record_violation(ctx, run_id, kind, message, details_json)`. Hàm này:
   - gọi `set_run_error`;
   - `merge_run_metrics` với `{"violation": {"kind": kind, ...details}}`.

   `kind` là một trong: `plan_invalid`, `files_outside_plan`, `wrong_branch`, `missing_plan`, `secret_leak`, `qa_extra_files`.
2. **Gọi helper ở mọi chỗ hiện chỉ `emit_out`.**

   | Chỗ | details |
   |---|---|
   | `roles/lead.rs:659-686` (plan_invalid lần 1) | `{"attempt":1,"errors":[...]}` |
   | `roles/lead.rs:687-698` (lần 2) | `{"attempt":2,"errors":[...]}` |
   | `roles/lead.rs:626-639` (file ngoài .plan.md) | `{"files":[...]}` |
   | `roles/lead.rs:642-646` (thiếu plan) | `{}` |
   | `roles/dev.rs:945-952` (sai nhánh) | `{"branch":"…","expected":"…"}` |
   | `roles/dev.rs:987-1013` (file ngoài plan) | `{"files":[...]}` |
   | `roles/mod.rs:1065-1097` (secret leak) | giữ `secret_leak`, thêm `runs.error` |
   | `roles/qa.rs:454-494` (QA đổi file ngoài write dirs) | `{"files":[...]}` |

   Danh sách file giới hạn 50 mục. Message nêu file nào và cách sửa. Ví dụ cho file ngoài plan: "N file ngoài plan: a, b… — thêm vào bảng Thay đổi hoặc mục 'Thư mục được ghi'".
3. **Lưu cảnh báo của run.** `RoleRun.warnings` (`roles/mod.rs:1121-1128`, `qa.rs:509`) phải được ghi vào `metrics_json.warnings` (mảng chuỗi) trước khi role trả về.
4. **Hiện trên dashboard.**
   - `sse.rs:63-79` `status_json` và JSON runs của task (`api_tasks.rs:2232-2265`) thêm `error` (chuỗi hoặc null) và `warnings` (mảng).
   - `assets/dashboard/js/task.js`: dưới mỗi run có lỗi hoặc cảnh báo thì hiện khối chữ. Dùng `textContent`, không dùng innerHTML.
5. **Hiện trên Telegram.**
   - `telegram/render.rs:403-413` (RunFailed) và `autopilot/view.rs:84-115` (ViPhamGiaoThuc, RunLoi): khi `runs.error` có giá trị, thay "xem log của run này" bằng tối đa 3 dòng đầu của lỗi, cắt 500 ký tự.
   - Chuỗi đã redact sẵn từ bước 1.
6. **Test.**
   - Từng loại vi phạm ở bảng trên ⇒ `runs.error` khác NULL và `metrics_json.violation.kind` đúng.
   - Plan_invalid lần 1 cũng có lỗi.
   - `status_json` có `error`.
   - render Telegram có dòng lỗi.

### A2. Cảnh báo plan có lệnh nhưng thiếu "Thư mục được ghi" (mục 20)

1. Thêm `planfile::plan_warnings(&plan) -> Vec<String>`. Các luật hiện có:
   - có mục `GRANT_SECTION_COMMANDS` mà không có `GRANT_SECTION_WRITE` thì cảnh báo: "Plan có 'Lệnh được chạy' nhưng không có 'Thư mục được ghi': file do lệnh sinh ra sẽ bị tính là ngoài plan".

   Chỉ dựa vào cấu trúc plan, không đoán theo tên lệnh.
2. Hiện cảnh báo ở các chỗ sau. Đây là cảnh báo, không chặn duyệt.
   - plan JSON của task (`api_tasks.rs:2167-2187`, trường `warnings`);
   - task.js, cạnh nút Duyệt;
   - tin Telegram chờ duyệt plan.
3. Test.

### A3. Vướng mắc đã xử lý (mục 13)

1. **`planfile::has_blockers` / `blocker_text`** (`planfile.rs:1097-1121`): tách mục Vướng mắc thành các dòng không rỗng (sau khi bỏ comment HTML).
   - Dòng dạng checkbox đã đánh dấu (`- [x]`, `- [X]`, `* [x]`) là **đã xử lý**.
   - Mục là vướng mắc khi còn ít nhất 1 dòng khác (dòng chữ thường hoặc `- [ ]`).
   - `blocker_text` chỉ trả các dòng chưa xử lý.
2. **Prompt.**
   - `assets/prompts/dev.md` và các prompt có nói về Vướng mắc: Dev ghi mỗi vướng mắc thành một dòng `- [ ] …`.
   - `assets/prompts/lead.md`: người duyệt hoặc Lead đánh `- [x]` khi đã xử lý.
3. **`claim::approve_check`** (`claim.rs:295-368`): plan ở commit sẽ duyệt còn vướng mắc chưa xử lý, và quy trình sẽ chạy tiếp ⇒ từ chối duyệt.
   - Lý do: "Plan còn N vướng mắc chưa đánh dấu [x]: …".
   - Như vậy API duyệt báo lỗi ngay, không trả `job_id` rồi dừng sau.
   - `taskstate::build_approve` hiện cùng lý do để nút bị khóa có giải thích.
4. **Test.**
   - Toàn `- [x]` ⇒ không phải vướng mắc.
   - Có một dòng thường ⇒ là vướng mắc.
   - Duyệt bị từ chối khi còn vướng mắc.

### A4. Ngân sách khi duyệt (mục 12)

1. **`claim::approve_check`**: tính `pricing::exhausted(budget_usage, BudgetLimits::from_workflow)` giống `decide.rs:391-392`.
   - Đã hết, không còn `bypass_left`, và quy trình sẽ chạy tiếp ⇒ từ chối.
   - Lý do: "Hết ngân sách quy trình (đã dùng X/Y phút, $A/$B) — gia hạn rồi duyệt lại".
   - `build_approve` hiện cùng lý do.
2. **Gia hạn ngân sách.** Thêm control `auto_extend_budget`:
   - API `POST /api/tasks/{id}/auto/extend-budget` với body `{minutes?: u32, usd?: f64, rev}`;
   - nút trên task.js;
   - nút trên Telegram khi stop `HetNganSach`, theo mẫu nút sẵn có và `has_control`.

   Phần gia hạn được ghi vào `auto_json` (thêm trường `budget_extra_minutes`, `budget_extra_usd`, có `#[serde(default)]`). `BudgetLimits` cộng thêm phần này.
   - Giới hạn mỗi lần gia hạn lấy từ config `autopilot.max_budget_extension_minutes` (mặc định 240) và `..._usd` (mặc định 20).
3. **Không tính thời gian chờ quota vào ngân sách.**
   - `pricing::budget_usage` (`pricing.rs:154-192`) bỏ qua run có outcome `quota` và outcome `env:down` (phase C2).
   - Run `timed_out` vẫn tính, vì đó là thời gian thật đã dùng.
4. **Test.**
   - Duyệt khi hết ngân sách ⇒ 409 với lý do.
   - Sau khi gia hạn thì duyệt được.
   - Run quota không bị tính.

### A5. Số vòng QA (mục 15)

**Nguyên nhân đã xác định từ DB.** Snapshot quy trình của task #17 có `stages: [plan, gate:you, implement, qa]` và `loops: []`, tức là không có bước `fix` và không có vòng `qa`. Khi đó nhánh `decide.rs:1275-1291` dừng với `cap: Some(1)`, nên `max_rounds` không bao giờ được dùng.

1. **Validate quy trình** (`workflows.rs` cạnh `:820-853`): `max_rounds > 1` mà không có bước `fix` hoặc không có vòng `qa` ⇒ **cảnh báo** (không phải lỗi, để giữ tương thích).
   - Nội dung: "max_rounds = N không có tác dụng vì quy trình không có bước fix / vòng qa".
   - Hiện ở trang sửa quy trình (`api_settings.rs` save/list trả về `warnings`) và lệnh CLI kiểm quy trình nếu có.
2. **Thông báo dừng ở nhánh không có vòng sửa** (`autopilot/view.rs` cho `HetVong` khi `cap == 1` và quy trình không có loop qa): "QA kết luận Cần sửa. Quy trình <name> không có vòng sửa tự động (thiếu bước fix / vòng qa)". Không ghi "sau 1/1 vòng".
3. **Test.** Quy trình không có fix/loop ⇒ thông báo mới. Quy trình có loop với `max_rounds=2` ⇒ dừng ở vòng 2.

Sửa cấu hình `e2e-tu-dong` là việc của người dùng hoặc điều phối viên, không nằm trong plan code.

### A6. Chuỗi đề xuất: dựa vào trạng thái cuối của task (mục 17)

Code ở `scheduler.rs:42-248`.

1. **Xóa khối lặp** `scheduler.rs:76-83`.
2. **Bỏ điều kiện theo run gần nhất** (`latest_run_status` ở `:105-111` và `:123-135`).

   Đề xuất kế tiếp chỉ bị đánh dấu thất bại khi task trước **đã kết thúc mà không thành công**:
   - `auto_state = 'ended'` và `status` không thuộc tập thành công; hoặc
   - task không còn tồn tại.

   Task đang `stopped` (còn chạy tiếp được) hoặc đang chạy ⇒ tiếp tục chờ. Ghi chú trạng thái chờ của đề xuất, cột hiện có hoặc `error` dạng thông tin: "Chờ task #N (đang dừng: <lý do>)".
3. **Tập trạng thái thành công.**
   - Định nghĩa một hằng duy nhất `tasks::SUCCESS_STATUSES = ["answered","qa_pass"]`.
   - Dùng lại ở `api_settings.rs:151` thay cho danh sách đang viết tay.
   - Chuỗi chỉ đi tiếp khi task trước ở trạng thái thành công.
   - Giữ luật hiện có: chỉ đề xuất `analysis` mới có `after` (`triage.rs:712-748`).
4. **Test.**
   - Run gần nhất `timed_out` nhưng task vẫn `stopped` ⇒ đề xuất vẫn chờ.
   - Task `ended` + `qa_fix` ⇒ đề xuất thất bại.
   - Task `answered` ⇒ đề xuất chạy.

### Commit phase A
`fix(agent-brain): persist run failure reasons, blocker checkboxes, budget and blocker checks at approve, chain waits on task end`

## Phase B: điều khiển task

### B1. Mở lại task đã xong (mục 18)

1. **Control `reopen`.**
   - API `POST /api/tasks/{id}/reopen` với body `{rev, note?: string}`.
   - Có trên task.js; trên Telegram khi QaPass, qua `has_control`.
   - Chỉ cho phép khi task đang ở trạng thái thuộc `SUCCESS_STATUSES`, không có run hay driver đang chạy, và nhánh task còn tồn tại.
2. **Hiệu ứng.**
   - `status` chuyển thành `qa_fix` với task build, hoặc `planned` với task analysis (chọn theo `workflow.kind`).
   - `auto_state` chuyển `stopped`, xóa `auto_json.stop`.
   - Ghi sự kiện hoặc lịch sử theo cơ chế audit sẵn có, kèm `note`.
   - Không tự chạy gì. Người dùng chọn tiếp Chạy Dev, Lead lại, hoặc Bật autopilot.
   - Bỏ khóa `auto_start` "Task đã xong" (`taskstate.rs:619-625`) cho task đã reopen.
   - `taskstate.rs:884-906` cấp action theo status mới.
3. **Test.** Reopen task `qa_pass` ⇒ có `run_dev` và `auto_start`. Reopen task chưa xong ⇒ 409.

### B2. Duyệt plan và chọn bước tiếp theo (mục 21)

1. **Body của `POST /api/tasks/{id}/approve`** thêm `next: "auto" | "dev" | "qa" | "none"`, mặc định `"auto"` (giữ nguyên hành vi cũ).
   - `none`: ghi duyệt, không chạy job, đặt `auto_state = stopped`. Giống CLI `approve --no-continue` (`autopilot/mod.rs:783-852`); dùng chung một hàm.
   - `qa`: chỉ cho phép khi nhánh đã có code sau plan (cùng điều kiện `code_after_plan` của `run_qa`). Ghi duyệt rồi chạy job QA. Nếu quy trình auto thì autopilot tiếp tục từ QA, không chạy Dev.
     - Cách làm: thêm `auto_json.skip_build_once: bool`. `decide.rs` rule D6 bỏ qua implement một lần khi cờ này bật và đã có commit code sau plan; dùng xong thì xóa.
   - `dev`: như hiện tại với quy trình không auto, tức là chạy Dev ngay.
2. **Giao diện.** task.js thêm chọn "Sau khi duyệt: tự động / chỉ Dev / thẳng QA / không chạy".
   - Telegram: thêm nút "Duyệt, không chạy". Chỉ thêm nút "Duyệt → QA" khi điều kiện QA thỏa.
3. **Test.** Mỗi giá trị `next`. `qa` khi chưa có code sau plan ⇒ 409.

### B3. Diff plan so với lần duyệt trước (mục 10)

1. **Migration DB mới** (tăng `SCHEMA_VERSION`): bảng `plan_approvals(id, task_id, commit, approved_at, source)`.
   - `claim::approve_record` thêm một dòng mỗi lần duyệt.
   - `clear_task_approval` không xóa bảng này.
2. **`GET /api/tasks/{id}/plan-diff`.**
   - Trả `{previous_commit, current_commit, diff}`, với diff là `git diff <prev> <cur> -- <đường dẫn plan>`. Chạy qua `gitops`, có giới hạn 64 KiB.
   - `previous_commit` là lần duyệt gần nhất khác commit hiện tại.
   - Không có lần duyệt trước thì trả `previous_commit: null`.
3. **Hiện diff.**
   - task.js: khối "Plan đổi so với lần duyệt trước" (có thể thu gọn) ngay trên nút Duyệt, bằng `textContent` và tô dòng +/−.
   - Telegram, tin chờ duyệt: thêm dòng "Plan đổi +a/−b dòng so với lần duyệt <sha7>".
4. **Test.** Duyệt, sửa plan, rồi diff có dòng mới. Lần đầu thì `previous_commit` là null.

### B4. Giao thẳng task khi orchestrator bật (mục 4)

`NewTask` (`api_tasks.rs:25-40`) thêm `level: Option<String>` và `direct: bool` (mặc định false).

1. **`direct = true`** ⇒ không gọi triage.
   - Tạo intake ở trạng thái `proposed` và một đề xuất trạng thái `approved` với dự án, quy trình, đội hình, độ khó, tiêu đề lấy từ body. Thiếu giá trị thì lấy mặc định của dự án, như `start_new_task` đang làm ở `:683-700`.
   - Chép inputs như luồng intake.
   - Cả việc trên nằm trong một transaction.
   - Scheduler sẽ chạy đề xuất theo `max_parallel` và luật bận như mọi đề xuất khác.
   - Trả `201 {intake_id, proposal_id, status:"approved"}`.
   - Dự án không tồn tại ⇒ 400. Hiện `project_row_id(...).ok()` đang âm thầm thành NULL.
2. **`direct = false`** mà body có `workflow`, `team` hoặc `level` ⇒ 400: "workflow/team/level chỉ dùng với direct=true".
   - Hiện code âm thầm bỏ qua các trường này.
   - Trước khi đổi, kiểm task.js (form tạo task) có gửi các trường này mặc định không. Có thì sửa form chỉ gửi khi người dùng chọn, và gửi kèm `direct`.
3. **Orchestrator tắt** ⇒ truyền `level` vào `StartRequest`, thay `level: None` ở `:1033`.
4. **Dashboard.** Form tạo task có ô "Giao thẳng, không qua phân loại" kèm chọn quy trình, đội hình, độ khó.
5. **Test.**
   - direct ⇒ có đề xuất `approved` và không gọi engine triage.
   - workflow khi không direct ⇒ 400.
   - level truyền tới task.

### B5. Không tách yêu cầu (mục 3)

1. **Migration**: cột `intake.split INTEGER NOT NULL DEFAULT 1`.
   - Cập nhật `INTAKE_COLUMNS`, `IntakeRow`, `insert_intake` (thêm tham số), và danh sách cột viết tay ở `orchestrator/mod.rs:12`.
   - Cập nhật mọi caller: `telegram/intake.rs:1252`, `fixbug/pull.rs:313`, `api_tasks.rs:985`, handoff `api_tasks.rs:2498`.
2. **`CreateIntakeBody`** (`api_intake.rs:76-83`) thêm `split: Option<bool>`.
   - Mặc định lấy từ config dự án `triage_split` (mặc định true).
   - Handoff luôn dùng `split=false`.
3. **`triage.rs`.**
   - `split = false` ⇒ prompt đổi luật L468–469 thành "Trả đúng 1 mục trong 'tasks', không dùng 'after'". Code cắt còn 1 mục, kèm warning nếu model trả nhiều hơn.
   - Thêm luật chung: intake có `project_id` thì đưa dự án đó vào prompt ("Người gửi đã chọn dự án <id>: giữ nguyên"). Hiện `project_id` không được gửi cho model.
4. **Test.** `split=false` với fake engine trả 3 mục ⇒ chỉ 1 đề xuất. Prompt có dòng dự án.

### Commit phase B
`feat(agent-brain): reopen task, approve with next step, plan diff since last approval, direct task, no-split intake`

Nếu diff lớn thì tách phần dashboard thành commit thứ hai: `feat(dashboard): ...`.

## Phase C: cấu hình chạy theo dự án, quy trình và engine

### C1. Timeout theo dự án/quy trình, và báo thời hạn cho agent (mục 9, 11)

1. **Cấu hình.**
   - Quy trình: `timeouts: {role: seconds}`. Dự án: `timeouts: {role: seconds}`.
   - Thứ tự ưu tiên: **dự án > quy trình > `config.timeout_seconds` > mặc định hiện có**.
   - Hàm mới `timeout_for_task(cfg, project, workflow, role)` cạnh `config.rs:965-982`.
   - Thay ở mọi caller: `lead.rs:520`, `dev.rs:860`, `qa.rs:398`, `review.rs:366`, `analyst.rs:292,401`. Triage và fastpath giữ nguyên.
   - Giới hạn 60–14400 giây.
   - Giao diện sửa: trang dự án và trang quy trình.
2. **Giới hạn một lệnh shell của engine.**
   - Mỗi engine trong config có trường tùy chọn `tool_timeout_seconds` (không có mặc định trong code).
   - `assets/config.default.json` có thể đặt giá trị cho engine claude. Agent kiểm tài liệu Claude Code về giới hạn Bash rồi ghi vào asset, không ghi vào Rust.
3. **Khối thời hạn trong prompt.** Thêm cho mọi vai, sinh từ giá trị thật:
   - "Bạn có N phút cho lần chạy này (hết lúc HH:MM). Hãy commit hoặc ghi kết quả trước khi hết giờ."
   - Khi engine có `tool_timeout_seconds`: "Mỗi lệnh shell bị cắt sau M phút. Lệnh dài hơn phải chia nhỏ, không chạy nền."
   - Ghi thêm `run_dir/deadline` (epoch ms).
   - Nối khối vào prompt theo cách `read_sources_prompt_block` đang làm ở `roles/mod.rs:492-498`.
4. **Lượt "chốt" khi Dev hết giờ.**
   - Quy trình có `finalize_on_timeout: {enabled: bool, minutes: u32}`, mặc định tắt.
   - Bật thì autopilot (`decide.rs` sau RunLoi do `timed_out` của Dev) chạy **một** lượt Dev tiếp nối nếu thỏa cả hai: còn file đổi trong phạm vi plan, và lượt chốt chưa chạy cho vòng này.
   - Lượt chốt có timeout `minutes` và `StageRun.note`: "Lượt chốt: không làm việc mới. Ghi kết quả đang có, cập nhật file kết quả theo plan, commit."
   - Dùng lại `continuation_reason` (`dev.rs:36-70`).
   - Lượt chốt không tính vào vòng.
5. **Test.**
   - Thứ tự ưu tiên timeout.
   - Prompt có khối thời hạn với số đúng; không có `tool_timeout_seconds` thì không có dòng shell.
   - Timeout kèm `finalize_on_timeout` ⇒ có lượt chốt với note; tắt thì giữ hành vi cũ.

### C2. Kiểm môi trường đích trước khi chạy (mục 19)

1. **Cấu hình dự án** `preflight`, danh sách tối đa 5 mục:
   ```json
   [{"kind": "http", "url": "…" | "url_env": "TÊN_BIẾN", "expect_status": [200,302], "timeout_seconds": 10},
    {"kind": "tcp", "host": "…", "port": 443, "timeout_seconds": 5}]
   ```
   Kèm `preflight_roles` (mặc định `["dev","qa"]`) và `preflight_retry_minutes` (mặc định 0 = không tự chạy lại).
   - `url_env` đọc biến môi trường của tiến trình serve, **không** đọc file `.env*`.
   - Log và thông báo không in URL đầy đủ, chỉ scheme://host.
2. **Thêm module `src/preflight.rs`.**
   - Kiểm HTTP bằng client sẵn có trong crate. Không có thì dùng TCP connect thuần và HTTP/1.1 tối giản. **Không** thêm dependency (không sửa `Cargo.toml`).
   - Trả `Ok` hoặc `Err(lý do)`.
3. **Gọi kiểm** trong `roles/mod.rs` `execute_role_staged`, sau `resolve_for_run_with` và trước khi tạo run, cho các vai trong `preflight_roles`.
   - Lỗi thì không tạo run. Thoát với mã riêng `EXIT_ENV_DOWN` (mã mới, theo cách các mã EXIT hiện có).
   - Autopilot gặp mã này thì dừng với `StopReason::MoiTruongChuaSan`, kèm lý do. Lần dừng này không tính vòng, không tính ngân sách.
4. **Tự chạy lại.** `preflight_retry_minutes > 0` ⇒ scheduler (cùng cơ chế với `handle_quota_stops`, `scheduler.rs:253-398`) kiểm lại theo chu kỳ, môi trường sống lại thì chạy `auto <task>`.
   - Telegram báo khi dừng và khi chạy lại.
5. **Kiểm định kỳ trong lúc chạy** (`preflight_interval_seconds`, mặc định 0 = tắt).
   - Runner kiểm theo chu kỳ, lỗi liên tiếp 2 lần thì dừng run với outcome `env:down`.
   - `decide.rs` `is_ok` coi `env:down` như quota: không tính vòng.
6. **Test.**
   - Server HTTP giả (TcpListener trong test) sống hoặc chết.
   - Chết ⇒ không có run và stop `MoiTruongChuaSan`.
   - Không có cấu hình preflight ⇒ y như cũ.
   - `url_env` không in URL.

### C3. Đổi đội hình khi hết quota chỉ sang đội hình sẵn sàng (mục 24)

Đã kiểm: agent-brain không tự đổi đội hình. Ở task #28, nút "Đội hình tiet-kiem" trên tin quota Telegram đã được bấm, nhưng engine của Dev trong đội hình đó thiếu key.

1. **Lọc nút đội hình trên tin quota** (`telegram/render.rs:504-519`): chỉ hiện đội hình mà engine cho vai của run bị dừng đã sẵn sàng.
   - Sẵn sàng nghĩa là `engine::readiness` trả `san_sang`, và `team_profile_refusal` không từ chối.
   - Với đội hình dạng phòng ban: có ít nhất một thành viên sẵn sàng cho vai và level đó (dùng lại bộ lọc của `departments::pick_from`).
   - Thứ tự `idx` và `team_list_ver` phải tính trên danh sách **đã lọc**, ở cả chỗ render lẫn chỗ giải mã callback (`bot.rs:1318+`), để callback không lệch.
2. **`quota_rerun`** (`api_tasks.rs:1943+`) kiểm cùng điều kiện, sau `team_profile_refusal`. Không sẵn sàng ⇒ từ chối với lý do từ readiness, ví dụ "Chưa đặt biến môi trường …".
   - Dashboard đổi đội hình dùng chung hàm này.
3. **Thời điểm tự chạy lại sau quota.** `handle_quota_stops`: nếu run có `metrics.quota.reset_at`, chạy lại sau `max(reset_at, stop_at + resume_after_minutes)`; không có thì giữ cách cũ.
4. **Trả lại đội hình cũ sau khi quota reset** (cấu hình `orchestrator.restore_team_after_quota`, mặc định false).
   - Khi `quota_rerun` đổi đội hình, ghi `reason:"quota"` và `reset_at` vào lịch sử đội hình.
   - Autopilot, ở ranh giới bước kế tiếp sau `reset_at`, trả đội hình về `from` của mục lịch sử đó, nếu đội hình cũ đã sẵn sàng.
5. **Test.**
   - Đội hình có engine thiếu key không xuất hiện trong nút.
   - `quota_rerun` sang đội hình đó ⇒ từ chối.
   - `reset_at` được dùng.

### C4. Repo nguồn, engine của Dev, và cwd của agy (mục 5, 22, 23)

1. **Block "Dự án nguồn"** (`projects.rs:176-193`): thêm câu "Mọi vai của task (Lead, Dev, QA, Review) đều đọc được các thư mục này bằng đường dẫn tuyệt đối."
   - Sửa test liên quan trong `tests/project_read_sources.rs`.
2. **Lead biết engine của Dev** (mục 22).
   - Thêm hàm xem trước, không có tác dụng phụ, ví dụ `engine::preview_for_role(ctx, task, "dev")`. Hàm dùng cùng logic với `resolve_for_run_with` và `departments::pick_from`, nhưng không ghi gì.
   - Lead nhận khối "Dev dự kiến chạy bằng engine <kind> (<thành viên>)" cùng nội dung `assets/prompts/engine-notes/<kind>.md` nếu file tồn tại.
   - Tạo `engine-notes/agy.md`: agy chỉ đọc được file trong "Thư mục được đọc"; cần khai báo mọi script hay thư viện mà lệnh của Dev sẽ đọc.
   - Không viết luật này trong Rust.
3. **Ghim engine/thành viên theo dự án.**
   - `projects_map.json` thêm `role_members: {"dev": "<member id>"}` hoặc `role_engines: {"dev": "<engine name>"}`, chỉ một trong hai cho mỗi vai.
   - `resolve_for_run_with` dùng giá trị ghim trước `departments::pick`.
   - Giá trị ghim chưa sẵn sàng ⇒ lỗi rõ ràng, không âm thầm rơi về chọn tự động.
   - Có endpoint PUT và giao diện trên trang dự án.
4. **cwd của agy** (mục 23).
   - (a) Tạo lại fake agy cho test, `tests/fakes/fake_agy.py`. File này được `tests/engine_agy.rs:17-24` tham chiếu nhưng không có trong repo.
     - Fake ghi ra stream `init.cwd` bằng **cwd thật** của tiến trình (`os.getcwd()`) và danh sách `--add-dir` nhận được.
     - Kiểm vì sao các test agy vẫn pass khi thiếu file. Nếu test tự bỏ qua thì bỏ cơ chế đó.
   - (b) Thêm cờ `--add-dir <project root>` **đứng đầu**, trước các thư mục nguồn, trong `agy_plan` (`engine.rs:912-921`).
     - `agy --help`: "Add a directory to the workspace (repeatable)", nên workspace chính phải là repo dự án.
     - Đồng thời giữ `current_dir` là project root.
   - (c) Phát hiện cwd sai: guard agy (`guard.rs:383-425`) đọc `init.cwd` từ stream. Khác project root (sau canonicalize) ⇒ dừng run, lỗi ghi qua A1 (`kind: "wrong_cwd"`).
   - (d) Test với fake: dự án có read_sources và Dev là agy ⇒ argv có root trước nguồn, và `init.cwd` khớp root. Fake báo cwd khác ⇒ vi phạm `wrong_cwd`.
   - (e) Sau phase C, Claude kiểm bằng agy thật trên repo nháp (ngoài plan agy). Nếu (b) chưa đủ thì ghi vào findings.

### Commit phase C
`feat(agent-brain): per-project timeouts and deadline notice, environment preflight, ready-only quota team switch, dev engine preview and pin, agy workspace root`

## Phase D: tách không gian làm việc theo task (mục 25, 26, 27)

### D1. Đường dẫn file giao thức ở một chỗ, có thể cấu hình (mục 27)

1. **Module `src/protocol_paths.rs`** (hoặc mở rộng `stream.rs`). Hàm `paths(project, task_slug) -> ProtocolPaths { plan, qa_report, plan_review, code_review }`, mỗi trường là đường dẫn tương đối so với gốc repo.
   - Cấu hình dự án `protocol_dir`. Mặc định `""` = gốc repo như hiện nay. Ví dụ `".brain/{slug}"`, trong đó `{slug}` là biến duy nhất được thay.
   - Giá trị phải là đường dẫn tương đối, không chứa `..`, không trỏ vào `.git`.
2. **Thay mọi chỗ viết cứng.** Danh sách trong báo cáo khảo sát:
   - `tasks.rs:20`, `fastpath.rs:245`, `lead.rs` (16 chỗ), `dev.rs` (14), `qa.rs` (16), `review.rs`, `human.rs:117,265`, `claim.rs:352`, `api_tasks.rs:1325,1332,2194`, `taskstate.rs` (12), `guard.rs` (11), `telegram/summary.rs:218`, `report.rs:3`, `stream.rs:7`, `roles/mod.rs:75`.
   - Mọi chuỗi `git show {rev}:.plan.md` dùng đường dẫn từ module.
   - Sau khi sửa, lệnh `grep -rn '"\.plan\.md"\|\.qa-report\.md' src/` chỉ còn kết quả trong module này và trong test.
3. **Prompt.**
   - `assets/prompts/{lead,dev,qa,plan-review,code-review,lead-untrusted}.md` và `skills.rs` thay tên file cứng bằng biến mẫu (`{plan_path}`, `{qa_report_path}`…).
   - Thay biến ở chỗ prompt được dựng. Nếu chưa có cơ chế biến mẫu thì thêm một hàm thay chuỗi đơn giản và test nó.
4. **Test.** `protocol_dir=".brain/{slug}"`:
   - Lead ghi và commit plan ở đó;
   - Dev và QA đọc đúng;
   - guard chặn ghi nhầm;
   - mặc định thì y như cũ.

### D2. Worktree riêng cho mỗi task (mục 25, 26)

1. **Cấu hình dự án** `workspace`: `"shared"` (mặc định, như hiện nay) hoặc `"worktree"`. Kèm:
   - `base_branch`: mặc định là nhánh hiện tại của repo lúc tạo task. Thay chữ `"main"` viết cứng ở `api_tasks.rs:851`.
   - `max_parallel_tasks`: số task chạy song song trong dự án ở chế độ worktree, mặc định 1.
2. **Chế độ worktree.**
   - `start_new_task` tạo nhánh `task/<slug>` từ `base_branch`, rồi `git worktree add <data_dir>/worktrees/<project_id>/<slug> task/<slug>`.
   - Thêm hàm trong `gitops.rs`. Đường dẫn worktree lưu ở cột mới `tasks.workdir` (cần migration).
   - Hàm `task_root(project, task)` trả `workdir` nếu có, ngược lại trả `project.root()`. Mọi vai, guard, gitops và taskstate dùng `task_root` thay cho `project.root()` khi làm việc của task.
   - Liệt kê và sửa từng chỗ. Gợi ý tìm: `grep -rn "\.root()" src/roles src/autopilot src/dashboard src/guard.rs src/human.rs`.
   - Kiểm "đang ở nhánh task" luôn đúng trong worktree. Kiểm working tree bẩn áp lên worktree của task, không phải repo chính.
   - `busy_reason` ở chế độ worktree tính theo task: cho phép tối đa `max_parallel_tasks` task có run hoặc driver đang chạy trong cùng dự án.
   - Repo nguồn chỉ đọc vẫn dùng đường dẫn dự án nguồn như cũ.
3. **Chế độ shared: sửa nhẹ hai lỗi.**
   - (a) Cấu hình dự án `hold_project_at_gate`, mặc định **true**: task ở cổng duyệt (`auto_state='gate'`) cũng tính là bận trong `busy_reason` và `claim::project_busy_reason`, nên Lead của task khác không chạy chen vào (mục 26).
   - (b) Duyệt khi repo không ở nhánh task, working tree sạch, và cấu hình `auto_checkout_on_approve` (mặc định true) ⇒ agent-brain tự `git checkout <nhánh task>` rồi duyệt.
     - Working tree bẩn thì giữ lỗi 409 như cũ, kèm danh sách file bẩn.
     - Sửa ở `taskstate::build_approve` (`:515-553`) và đường CLI `autopilot/mod.rs:812-825`.
4. **Dọn worktree.** Lệnh CLI `agent-brain task cleanup [<id>|--merged]` và nút trên task.js.
   - Chỉ xóa worktree của task ở `SUCCESS_STATUSES`, hoặc task đã kết thúc, khi nhánh đã merge vào `base_branch` **hoặc** người dùng xác nhận.
   - Dùng `git worktree remove` không có `--force`. Worktree bẩn thì từ chối.
   - Không bao giờ xóa nhánh.
5. **Lệnh merge task** (tùy chọn, nếu còn thời gian thì làm):
   - `agent-brain task merge <id>`: `git merge --no-ff task/<slug>` vào `base_branch` trên repo chính.
   - File giao thức xung đột thì giữ bản của `base_branch`, hoặc bỏ nếu không cấu hình `protocol_dir`. Xung đột khác thì abort và báo.
   - Chỉ cho task QA đạt. Không push.
6. **Test** (repo git tạm trong test):
   - worktree được tạo, Lead và Dev chạy trong worktree, repo chính không đổi nhánh;
   - hai task song song khi `max_parallel_tasks=2`;
   - shared mode với `hold_project_at_gate`: task thứ hai chờ;
   - `auto_checkout_on_approve`: sạch thì checkout và duyệt; bẩn thì 409;
   - cleanup từ chối worktree bẩn;
   - mặc định (shared, không tùy chọn) thì mọi test cũ pass.

### Commit phase D
1. `refactor(agent-brain): single source for protocol file paths, configurable protocol dir`
2. `feat(agent-brain): per-task git worktree, hold project at plan gate, checkout task branch on approve`

## Không sửa

- `Cargo.toml` (không thêm dependency).
- `~/.config/agent-brain/*` và `~/.local/share/agent-brain/*` thật.
- Dashboard thật trên cổng 7437. Serve thật đang chạy, KHÔNG dừng nó.
- Repo ngoài: apec, automation-test-e2e.
- `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` khác, `~/.claude-orca`.
- Không đổi `has_control` (`bot.rs:557`) ngoài việc thêm control mới theo đúng mẫu.

## Cách kiểm tra (mỗi phase)

```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'   # ≥ số trước phase + test mới, failed 0
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # không tăng so với trước phase (hiện 152)
grep -rnE 'apec|automation-test-e2e|e2e-tu-dong|"tiet-kiem"|deepseek' src/ | grep -v '^src/.*test' # không có kết quả mới
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
bash spikes/lib/scan-secrets.sh
```

Thêm cho phase D: `grep -rn '"\.plan\.md"\|\.qa-report\.md' agent-brain/src/` chỉ còn kết quả trong module đường dẫn giao thức.

## Commit và tóm tắt

- `git add` đúng file, không dùng `-A`, không `--no-verify`, không push.
- Mỗi commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Tóm tắt mỗi phase gồm:
  - file đã sửa và commit;
  - tùy chọn cấu hình mới (tên, chỗ đặt, mặc định);
  - chỗ lệch plan;
  - clippy trước và sau;
  - kết quả các lệnh kiểm tra.

## Việc của người dùng hoặc điều phối viên (không thuộc agy)

- Quy trình `e2e-tu-dong`: thêm bước `fix` và vòng `qa` để `max_rounds` có tác dụng (A5).
- Sau phase C:
  - đặt `timeouts.dev` và `preflight` cho dự án e2e;
  - cân nhắc `finalize_on_timeout`;
  - bỏ dòng "Dev có 120 phút…" viết tay trong `prompts/projects/automation-test-e2e/lead.md`, vì đã có khối thời hạn tự sinh.
- Sau phase D: cân nhắc chuyển dự án e2e sang `workspace: "worktree"` và `protocol_dir: ".brain/{slug}"`.
- Ghi chú prompt e2e (mục 6, 14, 16): giới hạn số lần chạy lại, đặt tên session theo task, dùng `screen-at-failure.txt` thay cho ảnh chụp sau khi đăng nhập.
