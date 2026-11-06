# Plan: Dev chạy tiếp được sau một lần Dev thất bại / quá giờ / bị dừng (không kẹt vì working tree bẩn)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (2026-11-06, dự án `srt`, task 12 `dung-1-video-1-2-phut-de-huong-dan-cach-giai`, đội `antigravity-full`)
- Run 63 (Dev, agy) sửa/tạo file trong phạm vi plan rồi bị agy tự dừng vì `--print-timeout` (status `failed`, exit 3). Các file đã sửa chưa được commit: `scripts/project_config.py`, `scripts/ga_va_cho_assets.py` (cả hai trong bảng Thay đổi), `projects/ga-va-cho-36-con/**`, `.gsd/phases/ga-va-cho-36-con/**` (thư mục được ghi).
- Autopilot chạy lại Dev → `roles/dev.rs` (~dòng 288–320) thấy working tree bẩn. Nhánh cho phép bẩn (`continuation`) chỉ bật khi `quota_continuation` thấy run Dev gần nhất có outcome quota. Run 63 là `failed`, nên mọi lần chạy lại đều bị từ chối với thông báo `Working tree có thay đổi chưa commit — hãy commit thay đổi tay với message '<task>: plan'` (thông báo này còn sai: đó không phải thay đổi của plan).
- Hệ quả: task nào có Dev hỏng giữa chừng đều kẹt, phải sửa tay trong repo dự án.

## Mục tiêu
1. Dev được chạy tiếp trên working tree bẩn (cùng luật lọc như sau quota: chỉ file trong plan, file giao thức, `.handover.md`, và file trong thư mục được ghi) khi run Dev gần nhất của task trên nhánh hiện tại:
   - có outcome quota (như cũ), HOẶC
   - có status `failed`, `timed_out`, `killed` hoặc `lost`.
   Không áp khi run gần nhất `succeeded` (khi đó bẩn là bất thường, giữ thông báo từ chối).
2. Khi chạy tiếp vì lần trước hỏng (không phải quota), thêm vào tin nhắn đầu vào của Dev một câu: "Lần chạy Dev trước bị dừng giữa chừng (<status>). Các thay đổi chưa commit trong phạm vi plan là của lần đó: kiểm tra, giữ phần đúng, làm tiếp phần còn thiếu; không làm lại từ đầu nếu không cần." Với quota giữ nguyên hành vi hiện có (gồm `.handover.md` nếu có).
3. Thông báo từ chối khi vẫn bẩn ngoài phạm vi: đổi thành `Working tree có thay đổi chưa commit ngoài phạm vi plan — commit hoặc bỏ các file này rồi chạy lại:` + danh sách. Bỏ cụm "với message '<task>: plan'".
4. Đổi tên hàm `quota_continuation` thành `continuation_reason` trả `Option<ContinuationReason>` (`Quota` | `PreviousRunFailed(String status)`), cập nhật chỗ gọi.

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/roles/dev.rs` | Mục tiêu 1–4 |
| 2 | `agent-brain/tests/cli_dev.rs` | Test |

Không sửa: `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dự án `srt`, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác, `.claude/worktrees/` khác.

## Test (bắt buộc, repo giả trong thư mục tạm)
- (a) Run Dev gần nhất `failed`, working tree có file trong bảng Thay đổi và file trong thư mục được ghi chưa commit → Dev chạy được; tin nhắn đầu vào có câu ở Mục tiêu 2 với status.
- (b) Như (a) với `timed_out`, `killed`, `lost` → chạy được.
- (c) Như (a) nhưng có thêm một file ngoài phạm vi plan bẩn → từ chối, thông báo mới (không còn "'<task>: plan'"), liệt kê đúng file đó.
- (d) Run Dev gần nhất `succeeded` + working tree bẩn → từ chối như cũ (thông báo mới).
- (e) Quota: hành vi và test cũ giữ nguyên.
- (f) Toàn bộ test cũ pass (cập nhật test nào đang khẳng định thông báo cũ, ghi rõ trong tóm tắt).

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"   # không tăng so với base
git diff --stat <base>..HEAD
```

## Commit
Một commit `fix(agent-brain): Dev continues after a failed, timed-out or killed Dev run`, add đúng file, không `--no-verify`, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: file sửa, commit, chỗ lệch plan, kết quả kiểm tra.
