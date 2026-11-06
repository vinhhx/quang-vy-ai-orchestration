# Plan (bổ sung cho agy-finish-after-result-and-command-audit): agy Dev nhận quyền của plan, vì giờ đã được kiểm từng bước

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`. Làm tiếp trên nhánh worktree `worktree-agent-ab9dc5d064da14b14` (sau commit 949d0aa). Đọc toàn bộ file này trước khi làm.

## Vấn đề (review của Claude)
- Commit 949d0aa kiểm từng bước công cụ của agy Dev bằng `guard::agy_tool_decision`, và khi policy không có danh sách lệnh thì chỉ cho git.
- Nhưng `roles/dev.rs::resolve_dev_grants` (~dòng 152–157) trả `None` cho `EngineKind::Agy` (in `GRANTS_CLAUDE_ONLY_MESSAGE`), nên policy của agy Dev KHÔNG BAO GIỜ có lệnh/thư mục của plan.
- Hệ quả: mọi task skill-mode chạy bằng agy (ví dụ dự án `srt`: `.venv/bin/python scripts/build_video.py *`, ghi `projects/<x>/**`) sẽ bị dừng `protocol_violation` ngay lệnh đầu tiên. Đây là đường người dùng dùng để dựng video (đội `antigravity-full`), nên không merge được như hiện tại.
- Lý do cũ của việc chỉ áp grants cho Claude là "agy không có hook nên không áp được". Lý do đó không còn: 949d0aa đã kiểm từng bước.

## Mục tiêu
1. `resolve_dev_grants` trả grants của plan đã duyệt cho cả `EngineKind::Agy` (giữ nguyên `Dsh` như cũ: in thông báo, trả `None`). Đổi nội dung `GRANTS_CLAUDE_ONLY_MESSAGE` cho đúng: quyền áp cho engine claude và agy.
2. Với agy Dev có grants: `policy.json` của run chứa đúng các lệnh, thư mục đọc, thư mục ghi của plan, theo đúng cách đang làm cho Claude (dùng lại hàm ghi policy hiện có; không cần `settings.json` hook cho agy, chỉ cần `policy.json`).
3. Tin nhắn đầu vào của agy Dev nhận cùng `DEV_GRANTS_NOTE` + `command_not_found_note` như Claude.
4. `agy_tool_decision` dùng lệnh/thư mục từ policy: lệnh khớp mẫu `## Lệnh được chạy` → được phép; lệnh khác → vẫn bị chặn như 949d0aa. Quy tắc "chỉ git khi policy không có danh sách lệnh" giữ nguyên cho plan không khai grants.
5. Các kiểm tra sau run Dev đang có cho grants (phạm vi ghi theo `write_dirs`, `grant_prefixes`, bỏ qua file merge vào…) áp cho agy giống Claude. Đọc `roles/dev.rs` để chắc không còn nhánh nào chỉ áp cho Anthropic.

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/roles/dev.rs` | Mục tiêu 1, 3, 5 |
| 2 | `agent-brain/src/roles/mod.rs` hoặc `agent-brain/src/guard.rs` | Mục tiêu 2, 4 (chỉ khi cần) |
| 3 | `agent-brain/tests/engine_agy.rs`, `agent-brain/tests/cli_dev.rs` | Test |

## Test (bắt buộc, dùng fake agy)
- (a) Plan skill-mode giống `srt` (lệnh `.venv/bin/python scripts/build_video.py *`, đọc `scripts/**`, ghi `projects/x/**`), engine agy: `policy.json` chứa lệnh và thư mục đó; tin nhắn đầu vào chứa `DEV_GRANTS_NOTE`.
- (b) Fake agy Dev phát `run_command` `.venv/bin/python scripts/build_video.py --project projects/x --stage all` → không bị dừng; `python3 -c 'x'` → bị dừng `protocol_violation`; ghi `projects/x/out.txt` → được phép; ghi `scripts/a.py` → bị dừng.
- (c) Plan không có grants, engine agy → hành vi như 949d0aa (chỉ git).
- (d) Engine dsh vẫn in thông báo và không có grants. Toàn bộ test cũ pass (cập nhật test nào đang khẳng định `GRANTS_CLAUDE_ONLY_MESSAGE` cho agy, ghi rõ trong tóm tắt).

## Cách kiểm tra
```bash
cd agent-brain
cargo test --no-fail-fast
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"   # không tăng so với 949d0aa
git diff --stat 949d0aa..HEAD
```

## Không được đụng tới
`~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dự án `srt`, `.planning/*`, `.claude/worktrees/` khác. Không `git add -A`, không `--no-verify`. Không chạy agy thật.

## Commit
Một commit `fix(agent-brain): agy Dev gets the plan's grants, checked step by step`, kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: file sửa, commit, chỗ lệch plan, kết quả test.
