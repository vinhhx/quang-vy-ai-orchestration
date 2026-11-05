# Plan: Sửa tiếp sau review `agent-git-permissions` (lỗ hổng branch/tag, 3 test đỏ)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`, HEAD `c42983f`. Đọc toàn bộ file này trước khi làm. Plan gốc: `.planning/quick/agent-git-permissions.plan.md`.

## 1. Lỗ hổng: `git branch` / `git tag` tạo được ref (bắt buộc sửa)
Đã kiểm tra trên repo thật: `git branch --color foo`, `git branch --column foo`, `git branch --abbrev foo` đều **tạo nhánh** `foo`; `git tag --column t1`, `git tag --color t2` đều **tạo tag**. Lý do: với các tùy chọn này, giá trị chỉ được nhận qua dấu `=` (`--color=always`). Từ đứng sau là đối số positional.

Trong `src/guard.rs`, `check_branch` và `check_tag` lại coi từ đứng sau `--color`/`--column`/`--abbrev` là giá trị và bỏ qua nó. Vì vậy `positional_count` vẫn bằng 0 và lệnh được cho qua.

Sửa:
- `check_branch`: bỏ `abbrev`, `column`, `color` khỏi danh sách tùy chọn "nuốt" từ tiếp theo. Chỉ giữ `sort`, `format`, `merged`, `no-merged`, `contains`, `no-contains`, `points-at`.
- `check_tag`: bỏ `column`, `color` khỏi danh sách tương ứng.
- Thêm vào bảng bị từ chối trong `tests/guard_git.rs`:
  - `git branch --color foo`, `git branch --column foo`, `git branch --abbrev foo`, `git branch --no-color foo`, `git branch -q foo`, `git branch -i foo`
  - `git tag --column t1`, `git tag --color t2`, `git tag -n t3`
- Các lệnh sau vẫn phải được phép (thêm vào bảng được phép nếu chưa có): `git branch --color=always -a`, `git branch -a --sort=-committerdate`, `git branch --sort -committerdate -a`, `git tag -l --sort=-v:refname`.

## 2. Cập nhật 3 test đỏ theo hành vi mới (được phép sửa các file test này)
Chạy `cargo test --offline --locked --no-fail-fast`, hiện có 3 test đỏ:
- `tests/cli_e2e.rs` `p2_01_run_lead_prints_steps_and_records_run` (khoảng dòng 133–156): `expected_allowed` vẫn là danh sách cũ. Sửa thành danh sách mới, dựng từ `agent_brain::roles::lead::LEAD_ALLOWED_TOOLS`. Nếu hằng không pub ra ngoài crate thì liệt kê tay đúng thứ tự thực tế: `Read, Grep, Glob, Write(./.plan.md), Edit(./.plan.md), Bash(git fetch *), Bash(git fetch)`, rồi toàn bộ `GIT_READ_ONLY_TOOLS`, rồi hai rule tuyệt đối. Không nới lỏng phép so sánh: vẫn phải so bằng (`assert_eq!`).
- `tests/lead_rounds.rs` `invalid_plan_records_violation`:
  - Fake Lead ghi plan sai ở cả hai lần, nên phải có **2** run lead.
  - `leads[0]`: outcome `plan_invalid`; `leads[1]`: outcome `protocol_violation`.
  - Cả hai đều `succeeded`.
  - Stderr chứa `Lead đã tự sửa 1 lần`.
  - Thêm kiểm tra: hai run có cùng giá trị cột `round`. Đọc thẳng từ DB, theo cách `lead_runs` đang đọc.
- `tests/lead_rounds.rs` `autopilot_invalid_plan_stops_once`:
  - Phải có 2 run lead, run cuối có outcome `protocol_violation`.
  - `stop.reason` vẫn là `vi_pham_giao_thuc`, `stop.run_id` bằng id của run **cuối**.
  - Không có run lead thứ 3.
  - Nếu kịch bản fake (`by_role.lead`) chỉ có một phần tử: kiểm tra xem `fake_claude` lặp lại phần tử cuối hay báo lỗi khi hết. Nếu báo lỗi, thêm phần tử thứ hai giống phần tử đầu (`invalid_lead_entry()`).
- Không sửa code `src/` để làm test xanh, trừ mục 1. Nếu thấy bắt buộc phải sửa code, dừng lại và báo.

## Không được đụng tới
- `.planning/`, `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`, các repo dự án khác.
- `tests/assets_parity.rs`, các prompt.
- Không kill hay restart `agent-brain serve`, không dùng pkill/killall.
- Không dùng `git add -A`, `--no-verify`, không push.
- Không checkout, restore hay stash file mà bạn không sửa.

## Kiểm tra
Trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test guard_git --test cli_e2e --test lead_rounds --test cli_lead 2>&1 | grep -E "^test result|FAILED"
```
Mong đợi: **mọi** test pass, không còn dòng FAILED.

## Commit
Một commit: `fix(agent-brain): git branch/tag list-mode check ignores optional-value flags; update tests for Lead retry`, kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối
- File đã sửa.
- Hash commit.
- Kết quả nguyên văn các lệnh kiểm tra.
- Chỗ lệch plan, nếu có.
