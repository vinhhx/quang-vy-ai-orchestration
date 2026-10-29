# Plan: agy Lead không chạy được git → plan rỗng bị báo "vi phạm giao thức"

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (Rust crate ở `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Bối cảnh (đã điều tra, không cần điều tra lại)
Run Lead của agy (`~/.local/share/agent-brain/runs/apec/review-thay-doi-moi-nhat-hom-nay-xem-thanh-vien/20261028T104644702479Z-lead/`):
- Lead chạy `git status`, `git log -n 5 --stat` → agy headless tự từ chối quyền `command` (stderr: "a tool required the "command" permission that headless mode cannot prompt for").
- agy trả `status: SUCCESS`, `response: ""`. `roles/mod.rs` (khối "D-05") chỉ ghi `.plan.md` khi response không rỗng → `.plan.md` vẫn là khung trống.
- `roles/lead.rs` (~dòng 466-484): `validate_plan` fail → `set_run_outcome("protocol_violation")` → dashboard hiện "vi phạm giao thức". Nhãn sai: agy không vi phạm gì, chỉ không có output.
- Dev đã có outcome `agy:empty` (`roles/dev.rs`, `AGY_EMPTY_MESSAGE`); Lead thì chưa.
- Cơ chế allow-rule của agy chưa được kiểm chứng, nên KHÔNG dựa vào nó và KHÔNG dùng `--dangerously-skip-permissions`, `--mode` cho Lead.

## Cách sửa
A. Với engine agy, agent-brain tự lấy sẵn ngữ cảnh git chỉ-đọc và nhét vào prompt Lead, để agy không cần chạy lệnh.
B. Khi agy Lead trả response rỗng thì gắn outcome `agy:empty` (không phải `protocol_violation`), kèm thông báo đúng.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/roles/lead.rs` | hàm chạy Lead, chỗ dựng `user_input` (trước `RoleSpec`) | Nếu engine đã resolve của role lead có kind `Agy` (xem cách `roles/dev.rs` ~dòng 582 dùng `ctx.cfg.engines.kind_of(...)`; dùng cách tương tự để lấy engine id của Lead) thì nối vào `user_input` một mục `Ngữ cảnh git (chỉ đọc, agent-brain lấy sẵn — bạn không chạy được lệnh shell)` gồm: nhánh hiện tại, `git status --short`, `git log -n 20 --stat --date=iso` (dùng `crate::gitops::git(&proj_path, &[...])`). Cắt mỗi khối tối đa 16 KiB (cắt ở ranh giới ký tự UTF-8, thêm dòng `… (đã cắt)`). Lỗi git thì bỏ khối đó, không làm Lead fail. Engine không phải agy: không đổi gì. Tách phần dựng chuỗi thành hàm thuần `fn git_context_block(...)` để test. | agy không chạy được lệnh, cần cấp dữ liệu sẵn |
| 2 | `agent-brain/src/roles/lead.rs` | ngay sau khối `match role_run.outcome.status {...}`, trước `let mut extra = ...changed_paths` | Nếu engine của run là kind `Agy` VÀ `role_run.summary.agy_response` rỗng/chỉ khoảng trắng (hoặc `agy_status != SUCCESS`) VÀ `.plan.md` chưa đổi so với khung: `set_run_outcome(run_id, "agy:empty")`, in `LEAD_AGY_EMPTY_MESSAGE` bằng `crate::term::emit_out`, trả `Err(BrainError::error(LEAD_AGY_EMPTY_MESSAGE))`. Không đụng nhánh "sửa file ngoài .plan.md" (ROLE-01) và nhánh `validate_plan`. Kiểm tra tên field thật trong `obs.summary` / `RoleRun.summary` trước khi dùng. | Gắn nhãn đúng, autopilot dừng "vướng mắc" thay vì "vi phạm giao thức" |
| 3 | `agent-brain/src/roles/lead.rs` | đầu file (hằng số) | Thêm `pub const LEAD_AGY_EMPTY_MESSAGE: &str = "agy không trả kế hoạch nào — có thể bị từ chối quyền chạy lệnh (xem stderr.txt của run). Chạy lại hoặc đổi engine cho bước plan.";` | Thông báo riêng cho Lead |
| 4 | `agent-brain/src/autopilot/decide.rs` | khối `last_run.outcome == Some("agy:empty")` (~dòng 936-950) | `detail_line` chọn theo `last_run.role`: `"lead"` → `crate::roles::lead::LEAD_AGY_EMPTY_MESSAGE`, còn lại giữ `crate::roles::dev::AGY_EMPTY_MESSAGE`. | Dòng chi tiết không còn nói "không sửa file" cho Lead |
| 5 | `agent-brain/src/engine.rs` | `agy_plan`, nhánh chuỗi chỉ-đọc (~dòng 726-729) | Thêm một câu vào prompt chỉ-đọc: `Bạn không chạy được lệnh shell (bị từ chối quyền); dùng ngữ cảnh đã cho và công cụ đọc file. Nếu thiếu thông tin, vẫn phải trả file đầy đủ, không được trả rỗng.` Giữ nguyên các câu cũ. | Giảm khả năng agy kết thúc rỗng |
| 6 | `agent-brain/src/roles/lead.rs` (module `tests`) và `agent-brain/src/engine.rs` (module `tests`) | cuối module test | Test: (a) `git_context_block` cắt đúng 16 KiB và an toàn UTF-8, bỏ khối khi git lỗi; (b) prompt agy chỉ-đọc chứa câu mới và `chế độ chỉ đọc`, prompt Dev thì không; (c) test hiện có `agy_permission_only_for_dev` vẫn pass (không thêm `--mode`/skip flag cho Lead). Nếu `tests/cli_lead.rs` có fake agy, thêm một case: fake agy trả `response: ""` → exit khác 0, `outcome == "agy:empty"` (không phải `protocol_violation`). Nếu `tests/common` chưa có fake agy cho Lead thì bỏ case này và ghi vào summary. | Chốt hành vi |

## Không được đụng tới
- `.planning/STATE.md`, `.planning/ROADMAP.md`, mọi file `.planning/` đang untracked, `.claude/worktrees/`, `~/.claude-orca`.
- `agent-brain/src/dashboard/taskstate.rs` (đã ánh xạ `agy:empty`), `roles/dev.rs`, `roles/qa.rs`, `roles/mod.rs`, `quota.rs`, `guard.rs`.
- Không thêm `--dangerously-skip-permissions`, `--mode`, `--sandbox` cho agy Lead / reviewer. Không đổi logic ghi `.plan.md` ở `roles/mod.rs`.
- Không đổi `brain/` (Python cũ) và `agent-brain/assets/`.
- `git add -A`, `--no-verify`: cấm. Commit từng file theo đường dẫn.

## Cách kiểm tra
Chạy trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build 2>&1 | tail -20
cargo test --lib 2>&1 | tail -30
cargo test --test cli_lead --test dashboard_pipeline --test autopilot_driver 2>&1 | tail -30
cargo clippy --all-targets 2>&1 | tail -20   # không thêm warning mới
```
Acceptance:
1. Lead engine agy: prompt gửi cho agy chứa mục "Ngữ cảnh git" với status + log (kiểm bằng test của `git_context_block`).
2. Lead engine claude: `user_input` không đổi so với trước.
3. agy Lead trả response rỗng → run có outcome `agy:empty`, không phải `protocol_violation`; autopilot dừng `VuongMac` với dòng chi tiết là `LEAD_AGY_EMPTY_MESSAGE`.
4. Toàn bộ test hiện có vẫn pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Branch: `main` hiện tại (tạo nhánh `fix/agy-lead-empty` nếu repo yêu cầu không commit thẳng vào main).
- Commit theo task: `fix(agent-brain): ...`, kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối ghi: file đã sửa, commit, chỗ lệch so với plan (nếu có), kết quả từng lệnh verify.
