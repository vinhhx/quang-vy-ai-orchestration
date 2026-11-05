# Plan sửa tiếp: chỉ tắt autopilot đang dừng khi task qua QA (không tắt lúc run bắt đầu)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.
Plan trước: `.planning/quick/autopilot-stale-stop-and-lead-git-c.plan.md` (commit `649cba0`, `60ad391`).

## Vì sao sửa tiếp
Commit `649cba0` gọi `db::end_stopped_autopilot` trong `Observer::on_spawn` (`src/roles/mod.rs`), tức là tắt autopilot ngay khi **bất kỳ** run tay nào bắt đầu. Điều này phá thiết kế có sẵn: khi autopilot dừng, người dùng có thể chạy tay một bước (ví dụ Dev) mà tin Telegram "Autopilot dừng" vẫn giữ nút (Kill, Chạy thêm 1 vòng, Dừng hẳn).
Test hỏng (qua ở `a16958a`, hỏng ổn định từ `649cba0`):
```
cargo test --test telegram_actions running_run_adds_kill_button
```

## Cách sửa
Chỉ tắt autopilot đang dừng khi task **qua QA** bằng một run tay (driver autopilot đang chạy thì `auto_state = 'running'` nên điều kiện SQL không khớp, không bị ảnh hưởng).

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/roles/mod.rs` | Xoá nguyên khối `if let Some(tid) = self.plan.task_id { if crate::db::end_stopped_autopilot(...) ... }` mà commit `649cba0` thêm trong `on_spawn`. Sau khi xoá, `git diff a16958a -- agent-brain/src/roles/mod.rs` phải rỗng. |
| 2 | `agent-brain/src/roles/qa.rs` | Trong nhánh `else` (khoảng dòng 282–286), ngay sau `crate::db::set_task_status(&ctx.conn, tid, "qa_pass")?;` (bên trong `if let Some(tid) = task_id`), thêm khối dưới đây. |
| 3 | `agent-brain/src/db.rs` | Giữ nguyên `end_stopped_autopilot` và test `test_end_stopped_autopilot`. Chỉ sửa doc comment của hàm thành: "Task qua QA bằng một run tay khi autopilot đang dừng (không có driver): tắt autopilot (`ended`), xoá cờ dừng, giữ auto_json làm lịch sử. Một câu lệnh có điều kiện; trả true khi đã ghi." |

Khối thêm vào `qa.rs`:
```rust
if crate::db::end_stopped_autopilot(&ctx.conn, tid)? {
    crate::term::emit(&format!(
        "Autopilot của task {tid} đang dừng — task đã qua QA nên tắt autopilot"
    ));
}
```

Không sửa file nào khác (không sửa test `telegram_actions.rs`, không sửa prompt, dashboard, Telegram, autopilot).

## Không được đụng tới
- `.planning/STATE.md`, `.planning/ROADMAP.md`, các file `.planning/` chưa track, `~/.claude-orca`, `~/.local/share/agent-brain`, `~/.config/agent-brain`, thư mục `agent-brain/target-pre`.
- Không cài binary, không restart `agent-brain serve`, không sửa brain.db thật.
- Không `git commit --amend`, không rebase, không revert commit cũ: làm một commit mới.

## Kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build 2>&1 | tail -2
git diff a16958a -- src/roles/mod.rs        # phải rỗng
cargo test --test telegram_actions 2>&1 | grep -E "test result|FAILED|panicked"
cargo test --lib end_stopped_autopilot 2>&1 | grep -E "test result|FAILED"
cargo test --test cli_qa --test autopilot_driver --test dashboard_autopilot --test cli_autopilot 2>&1 | grep -E "test result|FAILED|panicked"
cargo test 2>&1 | grep -E "test result|FAILED|panicked" | sort | uniq -c
```
Flaky đã biết: `contract_guard::guard_contract_cases` (BrokenPipe), chạy lại riêng nếu lỗi.

## Tiêu chí nghiệm thu
- `running_run_adds_kill_button` qua; toàn bộ `cargo test` qua.
- `src/roles/mod.rs` giống hệt `a16958a`.
- Run QA tay ra `qa:pass` khi autopilot `stopped`, không driver → `auto_state = 'ended'`.

## Commit
Một commit, `git add` từng file theo tên (không `git add -A`, không `--no-verify`):
`fix(agent-brain): end a stopped autopilot only when a manual QA passes`

## Báo cáo cuối
File đã sửa, hash commit, kết quả các lệnh Kiểm tra, chỗ lệch plan.
