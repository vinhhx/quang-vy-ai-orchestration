# Plan: Lead khai quyền đọc cho task review, lập lại plan có xử lý Vướng mắc, sửa test parity

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (đã kiểm tra)
Task #5 (dự án everon: "Kiểm tra các push mới…") là task review: Dev chỉ ghi một file báo cáo, nhưng phải đọc nhiều file nguồn ở commit khác qua `git show <commit>:<file>`.
- Lead viết plan với bảng Thay đổi chỉ gồm file báo cáo, và yêu cầu "lấy số dòng thật bằng `git show d60fe0bea:<file>`". Plan không khai `## Lệnh được chạy` hay `## Thư mục được đọc`.
- Prompt Dev (`assets/prompts/dev.md`, dòng 1, 2 và 10) cấm đọc mọi file ngoài bảng Thay đổi. Dev làm đúng prompt: ghi Vướng mắc rồi dừng. Guard thật ra cho phép `git show/log/diff` mặc định (`src/guard.rs`, `GIT_SUBCOMMANDS`), nhưng prompt cấm.
- `assets/prompts/lead.md` có câu "Dev chỉ được phép đọc và sửa các file được liệt kê trong bảng Thay đổi". Prompt này không nói Lead phải khai quyền đọc khi Dev cần đọc thêm file. Phần nói về các mục quyền chỉ áp dụng "khi dự án dùng kỹ năng".
- **Lập lại plan làm mất Vướng mắc:** `src/roles/lead.rs` dòng 208 có `keep_plan = is_replan && (opts.review || note.is_some()) && plan_path.is_file()`. Bấm "Lập lại plan" mà không có ghi chú thì `.plan.md` bị ghi đè bằng khung mới, nên mục Vướng mắc Dev đã ghi bị xoá trước khi Lead kịp đọc.
- **Cơ chế quyền đã có sẵn:**
  - `src/planfile.rs`: `GRANT_SECTION_COMMANDS = "Lệnh được chạy"`, `GRANT_SECTION_READ = "Thư mục được đọc"`, `run_grants`, `validate_grants`, `grant_command_error`. Ví dụ `git show *` là hợp lệ.
  - `src/roles/dev.rs`: `resolve_dev_grants` đọc quyền từ plan đã duyệt; `DEV_GRANTS_NOTE` (dòng 77) được nối vào prompt Dev khi plan có quyền.
- **Test đang đỏ `prompts_equal_phase2`** (`tests/assets_parity.rs`): `agent-brain/assets/prompts/lead.md`, sau khi bỏ dòng git-rule, phải giống từng byte với `brain/prompts/lead.md`. Hiện file asset có thêm khối "Khi dự án dùng kỹ năng…" (4 dòng, từ commit 6d3e1c4), còn `brain/prompts/lead.md` thì chưa có.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/assets/prompts/lead.md` | Xem mục A |
| 2 | `brain/prompts/lead.md` | Bằng đúng file #1 sau khi bỏ dòng bắt đầu `- Chạy mỗi lệnh git riêng, trên một dòng` (đúng quy tắc của `without_git_rule` trong `tests/assets_parity.rs`). Sinh file này bằng script từ file #1, không gõ tay. |
| 3 | `agent-brain/src/roles/dev.rs` | Xem mục B |
| 4 | `agent-brain/src/roles/lead.rs` | Xem mục C |
| 5 | `agent-brain/src/planfile.rs` | Chỉ thêm, nếu cần: hàm `pub fn blocker_text(plan_text: &str) -> Option<String>` trả nội dung mục `## Vướng mắc…` đã trim; trả `None` khi mục rỗng hoặc không có mục. Tận dụng `parse_plan` hay cách tách section đang có; không viết parser thứ hai. |
| 6 | `agent-brain/tests/cli_lead.rs`, `agent-brain/tests/cli_dev.rs` (và test unit trong `planfile.rs` nếu thêm hàm) | Xem mục Test |

KHÔNG sửa `assets/prompts/dev.md`. Lý do: `brain/prompts/dev.md` cũng phải giống từng byte với nó, và quyền đọc thêm chỉ có ý nghĩa khi plan khai quyền, nên phần này đi qua `DEV_GRANTS_NOTE`.

### A. `assets/prompts/lead.md`
1. Thay câu `- Dev chỉ được phép đọc và sửa các file được liệt kê trong bảng "Thay đổi", vì vậy bạn phải liệt kê đầy đủ tất cả file cần sửa để hoàn thành yêu cầu.` bằng **đúng** đoạn sau:
```
- Dev chỉ được sửa các file trong bảng "Thay đổi" và mặc định chỉ được đọc các file đó, vì vậy bạn phải liệt kê đầy đủ tất cả file cần sửa.
- Nếu Dev cần ĐỌC thêm (không sửa) để làm được việc, phải khai trong .plan.md, nếu không Dev sẽ dừng ở Vướng mắc:
  + File hay thư mục trong worktree: mục `## Thư mục được đọc`, mỗi dòng một mục trong backtick, ví dụ `- \`app/Http/Controllers/**\`` hoặc `- \`composer.lock\``.
  + Nội dung ở commit hay nhánh khác, hoặc lịch sử git: mục `## Lệnh được chạy` với các lệnh git chỉ đọc cần dùng, ví dụ `- \`git show *\``, `- \`git diff *\``, `- \`git log *\``. Không khai lệnh git ghi (commit, checkout, merge, push…).
  + Task review, phân tích hay viết báo cáo (chỉ ghi file báo cáo) gần như luôn cần các mục này: hãy khai đúng những gì Dev phải đọc.
- Nếu .plan.md đang có nội dung ở mục "Vướng mắc" (Dev ghi ở lần chạy trước): đọc kỹ, sửa plan để giải quyết từng điểm (bổ sung thông tin, file hoặc quyền đọc), rồi xoá nội dung mục đó, chỉ giữ tiêu đề.
```
   Khi ghi vào file, các backtick bên trong phải là backtick thật, không có dấu `\` đứng trước. Dấu `\` trong khối trên chỉ để hiển thị markdown của plan này.
2. Câu `- Để trống mục "Vướng mắc" (Dev sẽ ghi vào đây nếu gặp khó khăn trong quá trình thực thi).` sửa thành `- Khi viết plan mới, để trống mục "Vướng mắc" (Dev sẽ ghi vào đây nếu gặp khó khăn trong quá trình thực thi).`
3. Khối "Khi dự án dùng kỹ năng (skills)…" giữ nguyên. Dòng git-rule giữ nguyên, chỉ được có đúng **một** dòng (test kiểm tra điều này).

### B. `src/roles/dev.rs`: `DEV_GRANTS_NOTE`
Thêm vào cuối hằng (trước dấu `"` kết thúc) đúng câu: ` Ngoại lệ cho quy tắc chỉ đọc file trong bảng Thay đổi: được đọc các file trong Thư mục được đọc và nội dung lấy qua các lệnh đã khai (ví dụ git show), chỉ để làm đúng việc trong plan.`
- Tìm mọi test so khớp nguyên văn hằng này (`grep -rn "DEV_GRANTS_NOTE\|Plan có khai báo Lệnh được chạy" tests src`) và cập nhật cho khớp.
- Không đổi logic cấp quyền.

### C. `src/roles/lead.rs`: lập lại plan giữ Vướng mắc
- Khi `is_replan` là true, `.plan.md` có trong thư mục và `blocker_text(nội dung .plan.md)` trả `Some(_)`, thì coi như `keep_plan = true`, kể cả khi không có `--review` hay ghi chú.
- Khi đó, nối thêm vào `user_input` (trong nhánh `keep_plan`) đúng câu: `\nPlan có mục Vướng mắc do Dev ghi ở lần chạy trước: giải quyết từng điểm trong plan (bổ sung thông tin, file hoặc khai Thư mục được đọc / Lệnh được chạy), rồi xoá nội dung mục Vướng mắc, chỉ giữ tiêu đề.`
- Không đổi hành vi khi không có Vướng mắc: vẫn ghi khung mới như cũ.
- Kiểm tra `plan_unchanged` (khoảng dòng 559): Lead không sửa gì thì vẫn bị coi là "plan không đổi" như hiện tại. Đọc code để chắc chắn so sánh dùng `initial_plan_text` của nhánh `keep_plan`.

## Test
- **`cli_lead.rs`** (theo kiểu các test lập lại plan đang có, dùng `fake_claude` và file ghi prompt):
  1. Lập lại plan khi `.plan.md` có Vướng mắc khác rỗng và không có ghi chú: prompt gửi engine có câu ở mục C, và `.plan.md` lúc engine bắt đầu chạy vẫn còn nội dung cũ (không bị thay bằng khung).
  2. Lập lại plan khi mục Vướng mắc rỗng và không có ghi chú: hành vi cũ (ghi khung mới, prompt không có câu ở mục C).
- **`cli_dev.rs`:** khi plan đã duyệt có `## Lệnh được chạy` với `` `git show *` `` và không có mục Skill nào, thì prompt Dev chứa câu ngoại lệ ở mục B, và `policy.json`/settings của run cho phép `git show` (theo cách các test grants hiện có kiểm tra).
- **Unit test `blocker_text`** (nếu thêm hàm): mục rỗng; có nội dung; không có mục; mục nằm cuối file.
- **`tests/assets_parity.rs` phải XANH** (đây là mục tiêu, không sửa file test này).

## Không được đụng tới
- `tests/assets_parity.rs`, `assets/prompts/dev.md`, `brain/prompts/dev.md`, `brain/prompts/qa.md`, các prompt khác.
- `.planning/` (kể cả file plan này), `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`, repo everon và mọi repo dự án khác.
- Không kill hay restart `agent-brain serve` (cổng 7437), không dùng pkill/killall. Không dùng `git add -A`, không dùng `--no-verify`, không push.

## Kiểm tra
Trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test assets_parity 2>&1 | grep -E "^test result|FAILED"
```
Ở gốc repo:
```bash
diff <(grep -v '^- Chạy mỗi lệnh git riêng, trên một dòng' agent-brain/assets/prompts/lead.md) brain/prompts/lead.md && echo parity-ok
bash spikes/lib/scan-secrets.sh
```
Mong đợi: **mọi** test pass, kể cả `prompts_equal_phase2`; scan-secrets OK.

## Tiêu chí nghiệm thu
1. `lead.md` có hướng dẫn khai `Thư mục được đọc` / `Lệnh được chạy` cho việc đọc thêm, và hướng dẫn xử lý Vướng mắc khi lập lại plan.
2. Lập lại plan mà không có ghi chú không còn xoá Vướng mắc của Dev; Lead được yêu cầu giải quyết rồi dọn mục đó.
3. Plan có quyền đọc thì prompt Dev cho phép đọc qua quyền đó.
4. Toàn bộ test xanh, `prompts_equal_phase2` hết đỏ.

## Commit
Một commit: `fix(agent-brain): Lead declares read grants for review work and replans from Dev blockers`, kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối
File đã sửa, hash commit, kết quả nguyên văn các lệnh kiểm tra, chỗ lệch plan (nếu có).
