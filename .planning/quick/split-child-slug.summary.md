# Summary: split-child-slug

- `9230d39` scheduler (agy): `child_slug_candidate` cắt slug cha để chừa chỗ cho `-c{idx}[-r{n}]` (≤ SLUG_MAX_LEN, ổn định qua slugify). `pick_child_slug` loại candidate trùng slug cha. `handle_split_items` kiểm tra nhánh trước khi start và chặn task con trùng task cha/task khác.
- commit kế tiếp, split retry: agy viết, cửa sổ agy đóng trước khi commit, Claude review rồi commit.
  - Có `db::reset_retryable_split_items`, `split::retry`/`cmd_retry`, subcommand `split retry <task_id>`.
  - Status task cha đặt lại `planned` (status sau khi plan được lập/duyệt), `auto_state=waiting_children`.
- Verify (Claude chạy lại): `cargo build --release` OK, `cargo test` passed 2115 / failed 0, scan-secrets OK.
- Lệch plan: không có.
