# Plan: chạy tay một bước thì tắt autopilot đang "dừng"; Lead không dùng `git -C`

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (đã kiểm tra trên brain.db thật, task #6 cnc-tech)
1. **Trạng thái autopilot cũ không được xoá.** Run 38 (Dev) kết thúc `dev:blocked`, autopilot ghi `auto_state = 'stopped'`, `auto_json.stop.reason = "vuong_mac"`. Sau đó người dùng chạy tay Lead (run 39), duyệt, Dev (run 40), QA (run 41, `qa:pass`). Task `status = qa_pass` nhưng `auto_state` vẫn `stopped`.
   Hậu quả trong `src/dashboard/taskstate.rs` (khoảng dòng 773–850): vì `auto_state == "stopped"`, banner là "Chờ bạn · vướng mắc" thay vì "QA: Đạt", nhóm là `waiting` thay vì `done`, và còn hiện nút "Chạy thêm 1 vòng" / "Dừng hẳn" của một lần dừng đã cũ.
2. **Lead gọi `git -C <gốc dự án> branch …`** (run 36) và bị guard chặn ("tùy chọn toàn cục của git -C không được phép"). Không nới guard. Chỉ nói rõ trong prompt Lead rằng thư mục làm việc đã là gốc dự án.
   (Đã kiểm: binary hiện tại cho Lead chạy `git branch -a -vv`, `git remote -v`, `git rev-list …`; lỗi "git branch không nằm trong danh sách được phép" lúc 12:44 là của binary cũ. Không cần sửa danh sách lệnh git.)

## Mục tiêu
- Khi một run mới bắt đầu cho một task mà autopilot đang `stopped` và không có driver (`auto_pid IS NULL`), tức là người dùng tự chạy tay, thì tắt autopilot của task: `auto_state = 'ended'`, xoá `auto_stop_at`, `auto_stop_kind`. Giữ nguyên `auto_json` (làm lịch sử, như nút "Dừng hẳn").
- Driver autopilot không bị ảnh hưởng: khi driver chạy run, `auto_state` là `running` và `auto_pid` có giá trị, nên điều kiện không khớp.
- Prompt Lead có thêm câu: thư mục làm việc đã là gốc dự án, gọi `git <lệnh>` trực tiếp, không dùng `git -C <thư mục>`.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/db.rs` | Mục A |
| 2 | `agent-brain/src/roles/mod.rs` | Mục B |
| 3 | `agent-brain/assets/prompts/lead.md` | Mục C |
| 4 | `brain/prompts/lead.md` | Sinh lại từ #3 bằng lệnh ở Mục C. Không gõ tay. |
| 5 | Test unit trong `agent-brain/src/db.rs` (hoặc `src/autopilot/claim.rs` nếu helper test ở đó tiện hơn) | Mục Test |

### A. `src/db.rs`
Thêm, cạnh `clear_auto_stop` (khoảng dòng 828):

```rust
/// Người dùng tự chạy một bước khi autopilot đang dừng (không có driver):
/// tắt autopilot của task (`ended`), xoá cờ dừng, giữ auto_json làm lịch sử.
/// Một câu lệnh có điều kiện; trả true khi đã ghi.
pub fn end_stopped_autopilot(conn: &Connection, task_id: i64) -> BrainResult<bool> {
    let changed = conn.execute(
        "UPDATE tasks SET auto_state = 'ended', auto_stop_at = NULL, auto_stop_kind = NULL, updated_at = ?1 \
         WHERE id = ?2 AND auto_state = 'stopped' AND auto_pid IS NULL",
        params![now_ms(), task_id],
    )?;
    Ok(changed == 1)
}
```

### B. `src/roles/mod.rs`, `Observer::on_spawn` (khoảng dòng 518–535)
Ngay sau `let run_id = crate::db::insert_run_with_engine(...)?; self.run_id = Some(run_id);`:

```rust
if let Some(tid) = self.plan.task_id {
    if crate::db::end_stopped_autopilot(&self.ctx.conn, tid)? {
        crate::term::emit(&format!(
            "Autopilot của task {tid} đang dừng — chạy tay nên tắt autopilot (bật lại bằng Bật autopilot)"
        ));
    }
}
```

Không sửa chỗ nào khác của autopilot (`claim.rs`, `driver`), dashboard hay Telegram.

### C. Prompt Lead
1. Trong `agent-brain/assets/prompts/lead.md`, dòng bắt đầu bằng `- Khi khảo sát: đọc file bằng Read` (khoảng dòng 21): thêm vào **cuối dòng đó** (cùng dòng, sau dấu chấm cuối) câu:
   ` Thư mục làm việc đã là gốc dự án: gọi thẳng `git <lệnh>`, không dùng `git -C <thư mục>` hay tùy chọn toàn cục nào khác.`
   Không đổi dòng bắt đầu `- Chạy mỗi lệnh git riêng, trên một dòng` (test `assets_parity` khoá dòng đó).
2. Sinh lại bản Python:
   ```bash
   cd /mnt/data/IDE/QuangVy-Orchestration
   grep -v '^- Chạy mỗi lệnh git riêng, trên một dòng' agent-brain/assets/prompts/lead.md > brain/prompts/lead.md
   ```
   Sau đó `git diff brain/prompts/lead.md` chỉ được khác đúng câu vừa thêm.

## Test
Thêm test unit (dùng helper `fresh_task`/`set_row`/`auto_of` có sẵn trong `src/autopilot/claim.rs` tests nếu đặt test ở đó):
1. `auto_state = 'stopped'`, `auto_pid = NULL`, có `auto_stop_at`: `end_stopped_autopilot` trả true; sau đó `auto_state = 'ended'`, `auto_stop_at`/`auto_stop_kind` NULL, `auto_json` không đổi.
2. `auto_state = 'running'`, `auto_pid = Some(1)`: trả false, không đổi gì.
3. `auto_state = 'stopped'`, `auto_pid = Some(1)`: trả false.
4. `auto_state = 'gate'` hoặc NULL: trả false.

Không cần test tích hợp mới; các test hiện có phải vẫn qua.

## Không được đụng tới
- `.planning/STATE.md`, `.planning/ROADMAP.md`, các file `.planning/` chưa track, `~/.claude-orca`, `~/.local/share/agent-brain`, `~/.config/agent-brain`.
- Không cài binary mới vào `~/.local/bin`, không khởi động lại `agent-brain serve`, không sửa brain.db thật.
- Không sửa `src/guard.rs` (không nới `git -C`).
- Không sửa dòng git-rule trong bất kỳ prompt nào.

## Kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build 2>&1 | tail -3
cargo test --lib end_stopped_autopilot 2>&1 | tail -5     # hoặc tên test đã đặt
cargo test --test assets_parity 2>&1 | tail -5
cargo test --test autopilot_driver --test dashboard_autopilot --test cli_autopilot 2>&1 | grep -E "test result|FAILED|panicked"
cargo test 2>&1 | grep -E "test result|FAILED|panicked" | sort | uniq -c
cargo clippy --all-targets 2>&1 | grep -E "^(warning|error)" | sort | uniq -c
cd .. && git diff --stat HEAD~2..HEAD   # chỉ các file trong bảng Thay đổi
```
Flaky đã biết: `contract_guard::guard_contract_cases` (BrokenPipe) — chạy lại riêng nếu lỗi.

## Tiêu chí nghiệm thu
- Chạy tay một bước khi autopilot `stopped` và không có driver: sau khi run bắt đầu, task có `auto_state = 'ended'`, dashboard không còn banner "Chờ bạn · <lý do cũ>"; task `qa_pass` vào nhóm `done`.
- Driver autopilot đang chạy (`running`, có `auto_pid`) không bị đổi trạng thái.
- `brain/prompts/lead.md` và `assets/prompts/lead.md` có câu về `git -C`; `assets_parity` qua.
- Toàn bộ `cargo test` qua (trừ flaky đã biết, chạy lại qua), clippy không thêm cảnh báo mới.

## Commit
Hai commit, `git add` từng file theo tên (không `git add -A`, không `--no-verify`):
1. `fix(agent-brain): manual run ends a stopped autopilot` — `src/db.rs`, `src/roles/mod.rs`, test.
2. `docs(agent-brain): Lead calls git from the project root, no git -C` — hai file `lead.md`.

## Báo cáo cuối
Liệt kê file đã sửa, hash các commit, kết quả các lệnh Kiểm tra, và mọi chỗ làm khác plan.
