# Plan: bỏ luật "QA phải dùng engine claude" — mỗi vai dùng engine nào cũng được

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Quyết định của người dùng (2026-10-29)
Người dùng tự chọn engine cho từng thành viên của đội, tùy công việc; không muốn ràng buộc nào do hệ thống áp đặt. Luật D-06/D-39 ("QA luôn là `claude` có sẵn") bị bỏ. Phần "engine `người` và `dsh` chỉ dùng cho `dev`" GIỮ NGUYÊN (đó là giới hạn kỹ thuật của luồng chạy, không phải luật chính sách).

Engine của QA đã đi qua cùng `engine::resolve_for_run` như mọi vai, nên chỉ cần bỏ phần kiểm tra.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/teams.rs` | bộ nạp đội hình, khối `if role == "qa" && engine != Engine::Claude { errors.push(... "QA phải dùng engine claude."); continue; }` (~dòng 223-226) | Xóa khối này. Các kiểm tra khác (engine không hợp lệ, người chỉ cho dev) giữ nguyên. Sửa comment đầu file (dòng 2: "QA luôn là claude (D-06)") cho khớp. | QA chọn engine tự do |
| 2 | `agent-brain/src/teams.rs` | các test nhắc luật QA (~dòng 718, và test nào khác khẳng định QA bị từ chối) | Đổi: đội hình có `qa: "claude-max"` (hoặc id engine hợp lệ khác) nạp thành công và `engine_for("qa")` trả đúng engine đó; QA `người:<mã>` vẫn bị từ chối bằng thông báo "Engine người chỉ dùng cho vai trò dev."; giữ test QA mặc định là `claude` khi không ghi. | Test khớp hành vi mới |
| 3 | `agent-brain/tests/dashboard_settings.rs` | dòng ~233 và ~485 | Đổi hai assert đang chờ `"QA phải dùng engine claude."`: lưu đội hình qua API với QA là engine khác `claude` thành công (không còn nằm trong `refusals`), và file `teams.json` có QA khác `claude` nạp không lỗi. Thêm một assert QA `người:<mã>` vẫn bị từ chối. | Test tích hợp khớp |
| 4 | `agent-brain/assets/dashboard/js/settings.js` | khối `if (this.tQa !== 'claude') { out.push({field:'qa', message:'QA phải dùng engine claude.'}) }` (~dòng 628); comment ~dòng 692 và ~916; danh sách lựa chọn engine cho QA (~dòng 889-920) | Xóa kiểm tra tQa. Ô chọn engine của QA dùng cùng danh sách engine như các vai khác, trừ `người` và `dsh` (vì chỉ dùng cho dev); đọc cách danh sách của các vai khác được dựng ở ~dòng 889-920 và dùng lại, không viết lại. Sửa comment nhắc "the only QA engine". | Màn Cài đặt cho chọn |
| 5 | `agent-brain/assets/dashboard/screens/settings.html` | dòng ~84 | Đổi câu gợi ý của QA thành: `QA kiểm thử kết quả code. Chọn engine nào cũng được; nên chọn engine mạnh để kiểm tra lại việc của engine rẻ hơn.` | Gợi ý không còn là ràng buộc |
| 6 | `agent-brain/README.md` | dòng ~352 (câu cuối "QA luôn là `claude` có sẵn…") và ~371 ("**QA luôn là `claude`**; …") | Xóa ý QA bắt buộc `claude`; giữ ý: `người:<mã>` và `dsh` chỉ cho `dev`, engine khóa profile không dùng được ở dự án sai profile. | Tài liệu |
| 7 | `.planning/phases/06-multi-engine-media-branch/06-CONTEXT.md` | sau dòng D-45 | Thêm `- **D-46 (revises D-06 and D-39, user 2026-10-29):** Any role, QA included, may use any configured engine; the user picks the engine per role per job. Only the technical limit stays: \`người\` and \`dsh\` are for \`dev\` only.` | Ghi quyết định |
| 8 | `agent-brain/tests/` (kiểm tra có test dựng lại asset JS, ví dụ `dashboard_assets.rs`, `scripts/dashboard-js-check.mjs`) | — | Nếu có test hoặc script kiểm `settings.js`/`settings.html` bằng chuỗi cũ ("QA phải dùng engine claude"), cập nhật theo hành vi mới. Nếu `assets/dashboard/app.css` cần build lại thì KHÔNG đổi CSS (không thêm class mới). | Không làm vỡ kiểm tra asset |

## Không được đụng tới
- Luật engine `người` / `dsh` chỉ cho dev; `profile_refusal`; `claude_config_dir`; quota; `workflows.rs` và các stage.
- `~/.config/agent-brain/*`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED"      # toàn bộ test
node ../scripts/dashboard-js-check.mjs 2>&1 | tail -10                  # nếu script chạy được; ghi vào summary nếu không
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/teams\.rs" | head
```
Acceptance:
1. Đội hình `{"lead":"claude-max","dev":"claude-max","qa":"claude-max","plan-review":"claude-max","code-review":"claude-max"}` nạp và lưu được qua cả `teams.json` lẫn API Cài đặt.
2. QA = `người:<mã>` vẫn bị từ chối; QA = `dsh` vẫn bị từ chối.
3. Không ghi `qa` thì mặc định vẫn là `claude`.
4. Toàn bộ test pass; không còn chuỗi "QA phải dùng engine claude" trong `src/`, `assets/`, `tests/`, `README.md`.

## Quy trình
- Commit: `feat(agent-brain): any engine for any role, QA included` (code, asset, test) và `docs: D-46 any engine for any role` (README, CONTEXT). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả verify.
