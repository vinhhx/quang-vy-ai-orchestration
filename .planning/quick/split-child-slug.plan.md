# Plan: slug việc con trùng slug task cha + lệnh chạy lại việc con (`split retry`)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`.

## Bối cảnh (lỗi thật, task 68 ngày 2026-11-14)

- Task cha 68 có slug `viet-test-case-va-test-playwright-cho-toan-bo-r2` (48 ký tự = `routing::SLUG_MAX_LEN`).
- `orchestrator/scheduler.rs::pick_child_slug` tạo `"<slug cha>-c1"` (51 ký tự) và kiểm tra nhánh `task/<candidate>` còn trống — nhánh này đúng là trống.
- Nhưng `dashboard/api_tasks.rs::start_new_task` lại `routing::slugify(name)`. `slugify` cắt về 48 ký tự rồi bỏ tới dấu `-` cuối, nên ra **đúng slug cha**. `find_task_by_branch` trả task 68 (`is_new_task=false`), `add_to_task` chép tài liệu đầu vào vào chính task 68, gặp trùng tên và trả `Invalid("Tên tài liệu đã có")`. Việc 1 bị `failed`, 19 việc sau bị `skipped`, task cha bị tổng hợp thành `split_done` mà không có task con nào.
- Hiện không có cách chạy lại các việc con đã failed/skipped.

## Việc 1: slug việc con luôn hợp lệ, duy nhất và khác task cha

File: `agent-brain/src/orchestrator/scheduler.rs` (`pick_child_slug`, `handle_split_items`).

1. Mọi `candidate` mà `pick_child_slug` trả về phải ổn định qua `slugify`, tức `routing::slugify(&candidate) == candidate`, và `candidate.len() <= SLUG_MAX_LEN`. Cách làm: hậu tố `suffix` = `-c{idx}` (hoặc `-c{idx}-r{n}`). Phần đầu `head` = slug cha cắt sao cho `head.len() + suffix.len() <= SLUG_MAX_LEN`. Cắt theo byte, slug chỉ chứa ASCII; bỏ `-` thừa ở cuối `head`. Nếu `head` rỗng thì dùng `task`.
2. Nếu candidate trùng slug cha thì coi như không trống. Phòng thủ thêm như mục 1.
3. Trong `handle_split_items`, khi `start_new_task` trả `Started { task_id }`, kiểm tra trước khi gọi `set_task_parent`/`set_task_request`/`set_split_item_started`:
   - nếu `task_id == item.parent_id`, hoặc task đó đã tồn tại từ trước (đã có `parent_id` khác, hoặc nằm trong `split_items` của task khác) thì **không** gán. Đặt item `failed` với lý do rõ ràng, ví dụ `"Slug việc con trùng task #<id>"`.
   - Đơn giản hơn: gọi `find_task_by_branch` cho `task/<child_slug>` TRƯỚC khi start. Nếu đã có hàng thì chọn slug khác. Cuối cùng vẫn giữ kiểm tra `task_id != parent_id`.
4. Unit test trong `scheduler.rs` (`#[cfg(test)]`; dùng `fresh_db()` có sẵn hoặc tách hàm thuần, ví dụ `fn child_slug_candidate(parent_slug, idx, rerun) -> String`, để test không cần repo git):
   - slug cha 48 ký tự `viet-test-case-va-test-playwright-cho-toan-bo-r2`, idx 1 → kết quả ≤ 48, kết thúc bằng `-c1`, khác slug cha, `slugify(x) == x`.
   - idx 20 và rerun `-r3` cũng thỏa các điều kiện trên.
   - slug cha ngắn (`abc`) → `abc-c1` (giữ hành vi cũ).

## Việc 2: `agent-brain split retry <task_id>`

Files: `agent-brain/src/split.rs` (hàm `cmd_retry` + hàm lõi `retry`), `agent-brain/src/cli.rs` hoặc nơi khai báo subcommand `split` (thêm `Retry { task_id }`), `agent-brain/src/db.rs` nếu cần helper.

Hành vi:
- Chỉ áp dụng cho task cha có `split_items`. Không có thì báo lỗi, exit ≠ 0.
- Chọn các item có `state IN ('failed','skipped')` **và** `child_task_id IS NULL` (chưa từng có task con). Item failed có task con thì giữ nguyên: v1 không chạy lại task con.
- Đặt các item đó về `state='waiting'`, `reason=NULL`.
- Nếu có ít nhất 1 item được đặt lại thì đưa task cha về chờ việc con: `auto_state='waiting_children'`, auto driver NULL. Đưa `status` về giá trị trước khi `aggregate` đặt `split_done`. Đọc `autopilot/mod.rs` (`Decision::Split`, nhánh approve gọi `split::materialize`) và `db` để xác định giá trị đó. Nếu không xác định được thì dùng trạng thái mà task có ngay sau khi duyệt plan (xem `cmd_approve`). Ghi rõ lựa chọn trong summary.
- Báo cáo gộp cũ trong `answers/` có thể giữ; `aggregate` sẽ ghi đè khi các việc con xong.
- In số item đã đặt lại. Không có item nào thì in thông báo và exit 0.
- Unit test trong `split.rs` (theo mẫu `fresh_task()`): item 1 failed (không child), item 2 skipped, item 3 failed có `child_task_id` → sau retry item 1 và 2 là `waiting`, item 3 giữ nguyên, task cha `auto_state='waiting_children'`, `status != 'split_done'`.

## Ngoài phạm vi

Không đổi `SLUG_MAX_LEN`, `slugify`, `start_new_task`. Không đụng `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` chưa track, `~/.claude-orca`, `~/.config/agent-brain`, `~/.local/share/agent-brain` (DB thật).

## Verify

```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --release 2>&1 | tail -3
cargo test 2>&1 | grep -E "^test result|FAILED|panicked" | tail -20
./target/release/agent-brain split --help   # có retry
bash ../spikes/lib/scan-secrets.sh 2>/dev/null || true
```

Tiêu chí đạt: build OK, `cargo test` 0 failed, 4+ test mới pass, `split retry` có trong help.

## Commit và summary

Mỗi việc một commit (`git add <file cụ thể>`, không `-A`, không `--no-verify`). Ví dụ: `scheduler: slug việc con không trùng task cha`, `split: lệnh retry việc con failed/skipped`. Viết `.planning/quick/split-child-slug.summary.md` gồm: file đã đổi, commit, giá trị status chọn cho retry, các chỗ lệch plan. Commit file summary.
