# Plan: thêm test tích hợp cho agy Lead (response rỗng + ngữ cảnh git)

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước. Chỉ thêm test, KHÔNG sửa code production.

## Bối cảnh
Plan trước (`agy-lead-readonly-context.plan.md`, commit `af626ea`, `fa01a7a`, `cbcd452`) đã:
- nối ngữ cảnh git (`## Ngữ cảnh git ...`) vào prompt khi Lead chạy bằng agy (`roles/lead.rs`);
- gắn outcome `agy:empty` khi agy Lead trả response rỗng/không SUCCESS và `.plan.md` chưa đổi.
Hai hành vi này mới có unit test, chưa có test chạy cả luồng. Harness đã có sẵn: `agent-brain/tests/engine_agy.rs` dùng fake agy `tests/fakes/fake_agy.py`, helper `AgyEnv`, `agy_lines`, `run_lead`, `run_row`, `setup_project`, `write_doi_agy`. Xem test mẫu `agy_lead_tracer`, `agy_env_and_argv_hygiene` (đọc argv/prompt từ file record) và `agy_dev_empty_diff_suspicious` (kiểm `outcome == "agy:empty"`).

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/tests/engine_agy.rs` | thêm cuối phần "Task 1: agy as Lead" (sau `agy_error_status_fails`) | Test `agy_lead_empty_response_is_agy_empty`: fake agy trả `agy_lines("", "SUCCESS", "request-review")`; chạy `run_lead`; assert exit code khác 0, `run_row(&sb,"lead").outcome == Some("agy:empty")` (không phải `protocol_violation`), `status == "succeeded"`, stdout/stderr chứa chuỗi `agy không trả kế hoạch nào`. Nếu fake agy không dựng được kịch bản "SUCCESS + response rỗng" bằng `AgyEnv::new`, dùng đúng dạng `{"lines": ...}` như `agy_lead_tracer`. | Kiểm cả luồng từ run đến outcome |
| 2 | `agent-brain/tests/engine_agy.rs` | ngay sau test ở dòng trên | Test `agy_lead_prompt_has_git_context`: chạy Lead agy thành công (như `agy_lead_tracer`), đọc prompt (`-p`) từ file record của fake agy theo cách `agy_env_and_argv_hygiene` làm; assert nó chứa `## Ngữ cảnh git`, `### git status --short`, `### git log -n 20 --stat --date=iso` và tên nhánh hiện tại. | Chốt việc nối ngữ cảnh git |
| 3 | `agent-brain/tests/engine_agy.rs` | cùng chỗ | Test đối chứng `claude_lead_prompt_has_no_git_context`: chỉ viết nếu harness đã có cách chạy Lead bằng claude giả trong file này hoặc `cli_lead.rs` (dùng `fake_claude`); assert prompt không chứa `## Ngữ cảnh git`. Nếu cần dựng hạ tầng mới thì BỎ test này và ghi vào summary. | Engine claude không đổi |

## Không được đụng tới
- Mọi file dưới `agent-brain/src/` và `agent-brain/assets/`; `tests/fakes/fake_agy.py` (trừ khi bắt buộc để dựng kịch bản — nếu phải sửa, ghi rõ trong summary và chỉ thêm, không đổi hành vi cũ).
- `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` untracked, `.claude/worktrees/`, `~/.claude-orca`.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn file.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo test --test engine_agy 2>&1 | tail -30
cargo test --lib 2>&1 | grep -E "^test result"
cargo clippy --all-targets 2>&1 | grep -E "tests/engine_agy.rs" | head   # không có cảnh báo mới
```
Acceptance: hai test mới (và test thứ ba nếu làm được) pass; toàn bộ test cũ trong `engine_agy.rs` vẫn pass; chỉ `tests/engine_agy.rs` (và `fake_agy.py` nếu bắt buộc) bị sửa. Để chắc test có ý nghĩa, tạm thời xác nhận test 1 FAIL nếu đổi `"agy:empty"` thành `"protocol_violation"` trong test (không commit thay đổi này).

## Quy trình
- Một commit: `test(agent-brain): cover agy Lead empty response and git context`, kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là test 3 làm hay bỏ), kết quả từng lệnh verify.
