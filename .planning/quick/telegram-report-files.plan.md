# Plan: agent-brain tự gửi file báo cáo qua Telegram khi task qua QA; Lead không dùng script Telegram của dự án

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (đã kiểm tra)
Task #6 (cnc-tech) có yêu cầu "… báo cáo qua telegram".
- Lead thấy skill `fix-bugs` của dự án có `.claude/skills/fix-bugs/scripts/notify.sh`, nên khai lệnh đó trong plan và bắt Dev gửi Telegram bằng bot riêng của dự án (`.env.fixbugs`). Dự án chưa có `.env.fixbugs`, nên Dev ghi Vướng mắc và dừng.
- Người dùng muốn **agent-brain** gửi báo cáo cho họ qua bot đã ghép, không phải dự án tự gửi.
- Hiện tại, khi task qua QA, `src/telegram/outbox.rs` (`notify`, khoảng dòng 265–360) gửi 1 file "Tóm tắt" (`telegram::summary::document`) rồi gửi tin nhắn. File báo cáo Dev viết (ví dụ `.gsd/reports/tien-do-ghep-code-2026-11-05.md`) không được gửi.

## Mục tiêu
1. Khi task qua QA (`EventKind::QaPass`), sau file Tóm tắt và trước tin nhắn, bot gửi thêm các **file báo cáo** của task, mỗi file một `sendDocument`.
2. File báo cáo được xác định như sau:
   - Nếu plan có mục `## Gửi báo cáo`: lấy đúng các file khai trong mục đó.
   - Nếu không có mục đó: lấy các file `.md` **mới tạo** trên nhánh task mà có trong bảng Thay đổi.
3. Prompt Lead: việc báo cáo cho người dùng qua Telegram là của agent-brain; Lead không khai script gửi tin của dự án cho việc này, mà khai file báo cáo trong `## Gửi báo cáo`.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/planfile.rs` | Mục A |
| 2 | `agent-brain/src/telegram/summary.rs` | Mục B |
| 3 | `agent-brain/src/telegram/outbox.rs` | Mục C |
| 4 | `agent-brain/src/gitops.rs` | Chỉ thêm hàm, nếu cần (Mục B) |
| 5 | `agent-brain/assets/prompts/lead.md` | Mục D |
| 6 | `brain/prompts/lead.md` | Sinh lại từ #5 bằng script: bỏ dòng bắt đầu `- Chạy mỗi lệnh git riêng, trên một dòng`. Không gõ tay. |
| 7 | `agent-brain/tests/telegram_notify.rs`, test unit trong `planfile.rs`, `summary.rs` | Mục Test |

KHÔNG sửa template/skeleton `.plan.md` (có test parity `templates_equal_phase2`). Mục `## Gửi báo cáo` là mục tuỳ chọn, Lead tự thêm.

### A. `src/planfile.rs`
1. Hằng `pub const REPORT_SECTION: &str = "Gửi báo cáo";`.
2. `pub fn report_files(plan: &Plan) -> Vec<String>`:
   - Đọc mục `REPORT_SECTION`; mỗi dòng dạng `` - `đường/dẫn` `` (có thể có chú thích sau backtick).
   - Lấy đường dẫn trong backtick đầu tiên, chuẩn hoá bằng `normalize_plan_path`.
   - Bỏ trùng, giữ thứ tự.
   - Tận dụng cách đọc mục grants (`run_grants`) đang có; không viết parser mới.
3. `validate_plan`: khi mục có mặt, mỗi file phải thoả các điều kiện dưới đây. Sai thì thêm lỗi tiếng Việt rõ ràng; lỗi này đi vào cơ chế Lead tự sửa 1 lần đã có.
   - Đường dẫn tương đối, không có `..`, không bắt đầu bằng `/` hay `~`.
   - Đuôi `.md`, `.txt` hoặc `.csv` (không phân biệt hoa thường).
   - Không phải `report::sensitive_path`.
   - Có trong bảng Thay đổi (`plan_files`).
   - Tối đa 5 file.

### B. `src/telegram/summary.rs`: chọn và đọc file báo cáo
Thêm `pub fn report_documents(s: &TaskState) -> Vec<(String, Vec<u8>)>`, trả `(tên file hiển thị, nội dung)`:
1. **Danh sách ứng viên:**
   - Plan có mục `Gửi báo cáo`: lấy `planfile::report_files`.
   - Không có: lấy các file **thêm mới** trên nhánh task, tức `git diff --name-only --diff-filter=A <base>..<nhánh task>`, với `base` là `s.task.base_commit` khi hợp lệ (`gitops::is_hex_sha`). Giữ những file đuôi `.md`, có trong `plan_files`, và không phải `.plan.md`, `.qa-report.md`, `.plan-review.md`, `.code-review.md`, `.handover.md`.
   - Không có base hợp lệ thì trả rỗng.
2. **Lọc an toàn** (áp dụng cho cả hai nguồn):
   - Không `sensitive_path`.
   - Đuôi `.md`/`.txt`/`.csv`.
   - Tối đa 5 file.
3. **Đọc nội dung** từ git của nhánh task, không đọc worktree: `git show <nhánh task>:<path>` (`s.task.branch`, chạy ở `s.root`). Thêm hàm đọc bytes vào `gitops` nếu chưa có.
   - Bỏ qua file không tồn tại ở nhánh.
   - Bỏ qua file lớn hơn 1 MiB và in `Telegram: bỏ file báo cáo <path> — lớn hơn 1 MiB` bằng `term::emit`.
4. **Tên file hiển thị:** tên gốc (basename). Hai file trùng basename thì thêm tiền tố thư mục cha, nối bằng `-`.
5. Không gọi LLM, không đọc diff.

### C. `src/telegram/outbox.rs`: gửi file báo cáo
Trong `notify`, nhánh `EventKind::QaPass`:
- Sau khi file Tóm tắt đã xong (`doc_done`), trước khi gửi tin nhắn, gửi lần lượt các file của `summary::report_documents(&s)`.
- Danh sách chỉ được tính **một lần** cho mỗi lần notify. Giữ chỉ số file đã gửi để khi retry không gửi lại file đã gửi.
- Mỗi file một `sendDocument`, dùng `Body::Upload` giống file Tóm tắt. Caption là `render::clip(&format!("Báo cáo task {task_id}: {tên file}"), CAPTION_MAX)`.
- Retry và bỏ qua theo đúng quy tắc của file Tóm tắt:
  - Lỗi có `retry_delay_ms` thì thử lại.
  - Lỗi không thử lại được thì in `Telegram: không gửi được file báo cáo <tên> cho task {task_id} — …` và chuyển sang file tiếp theo.
  - Trước mỗi file vẫn kiểm tra lại task (D-05) như hiện tại, bằng cách `continue` vòng lặp.
- Tin nhắn QA vẫn reply vào file Tóm tắt (`doc_message`), không đổi.
- Sửa câu lỗi cũ `không gửi được file plan` thành dùng đúng nhãn (`Tóm tắt`/`Plan`) nếu tiện. Không bắt buộc.
- Các sự kiện khác (Gate, Stop, RunFailed, Quota) không đổi.

### D. `assets/prompts/lead.md`
Thêm **ngay sau** dòng bắt đầu `- Mục \`## Lệnh được chạy\` chỉ được khai` đúng dòng sau. Backtick là backtick thật, không có `\`.
```
- Khi yêu cầu nói báo cáo hay gửi kết quả qua Telegram (hoặc "gửi cho tôi"): agent-brain tự gửi cho người dùng khi task qua QA. Dev chỉ viết file báo cáo (.md, .txt hoặc .csv, có trong bảng Thay đổi); bạn thêm mục `## Gửi báo cáo` liệt kê các file đó, mỗi dòng một file trong backtick, tối đa 5 file. KHÔNG khai script gửi tin của dự án hay skill (ví dụ notify.sh, telegram, curl) cho việc này.
```
- Dòng git-rule giữ nguyên.
- Sinh lại `brain/prompts/lead.md`.

## Test
- **`planfile`** (unit):
  - `report_files`: mục có 2 file, có chú thích sau backtick, có dòng trùng.
  - `validate_plan`:
    - Lỗi khi file không có trong bảng Thay đổi.
    - Lỗi khi đuôi `.php`.
    - Lỗi khi có `..`.
    - Lỗi khi quá 5 file.
    - Lỗi khi là `.env.md` hoặc đường dẫn mà `sensitive_path` bắt.
    - Hợp lệ với `.gsd/reports/x.md` có trong bảng Thay đổi.
- **`summary::report_documents`** (unit, repo tạm thật):
  1. Plan có `## Gửi báo cáo` với `reports/a.md`, đã commit trên nhánh task: trả 1 phần tử `("a.md", nội dung)`.
  2. Không có mục; nhánh task thêm mới `reports/b.md` (có trong Thay đổi), sửa `README.md` và thêm `src/x.php`: chỉ trả `b.md`.
  3. File lớn hơn 1 MiB: bị bỏ qua.
  4. Worktree có nội dung khác chưa commit: vẫn trả nội dung ở nhánh (đọc từ git).
- **`tests/telegram_notify.rs`** (theo kiểu `qa_pass_sends_summary_file_before_text`):
  - Task qua QA với plan có `## Gửi báo cáo` một file đã commit:
    - Có 2 `sendDocument`: Tóm tắt trước, báo cáo sau, caption bắt đầu `Báo cáo task 1:`.
    - Sau đó có 1 `sendMessage`.
  - Các test cũ đang đếm `sendDocument == 1` phải vẫn xanh. Repo test của chúng không có file `.md` mới; nếu có, sửa test cho đúng hành vi mới và ghi lại trong báo cáo cuối.
- **`tests/assets_parity.rs` phải xanh.** Mọi test khác phải xanh.

## Không được đụng tới
- `tests/assets_parity.rs`, template `.plan.md`, các prompt khác ngoài `lead.md` (hai bản).
- `.planning/`, `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`, các repo dự án (kể cả cnc-tech).
- Không kill hay restart `agent-brain serve`, không dùng pkill/killall.
- Không dùng `git add -A`, `--no-verify`, không push.
- Không checkout, restore hay stash file mà bạn không sửa.
- Không gửi tin Telegram thật trong test (dùng transport giả có sẵn).

## Kiểm tra
Trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test telegram_notify --test assets_parity 2>&1 | grep -E "^test result|FAILED"
```
Ở gốc repo:
```bash
diff <(grep -v '^- Chạy mỗi lệnh git riêng, trên một dòng' agent-brain/assets/prompts/lead.md) brain/prompts/lead.md && echo parity-ok
bash spikes/lib/scan-secrets.sh
```
Mong đợi: **mọi** test pass (không còn dòng FAILED), in ra `parity-ok`, scan-secrets OK.

## Tiêu chí nghiệm thu
1. Task qua QA có file báo cáo thì người dùng nhận Tóm tắt, rồi từng file báo cáo, rồi tin nhắn QA đạt.
2. Plan không có `## Gửi báo cáo` vẫn gửi các file `.md` mới tạo có trong bảng Thay đổi (trường hợp task #6).
3. Không gửi file nhạy cảm, file lạ đuôi, file quá 1 MiB, quá 5 file.
4. Prompt Lead hướng dẫn dùng `## Gửi báo cáo` thay cho script Telegram của dự án.

## Commit
Một commit: `feat(agent-brain): send task report files over Telegram when QA passes`, kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối
- File đã sửa.
- Hash commit.
- Kết quả nguyên văn các lệnh kiểm tra.
- Test cũ đã sửa (nếu có).
- Chỗ lệch plan, nếu có.
