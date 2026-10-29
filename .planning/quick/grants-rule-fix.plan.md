# Plan: sửa quy tắc kiểm tra "Lệnh được chạy" làm plan skill hợp lệ bị từ chối

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Lỗi thực tế (2026-10-29)
Task `video-srt-idea` của dự án `srt`: run Lead (run 11) lập plan đúng định dạng skill mode nhưng `planfile::validate_plan` từ chối → outcome `protocol_violation`. Đã tái hiện bằng test tạm trên `.plan.md` thật:
1. Lệnh `python scripts/parse_srt.py projects/ganh-nang-tren-vai/input/subtitles.srt*` bị từ chối: ``*` chỉ được là từ cuối cùng của lệnh` (đúng quy tắc, nhưng thông báo khó hiểu cho Lead).
2. Bỏ `*` đi, lệnh `python scripts/parse_srt.py projects/ganh-nang-tren-vai/input/subtitles.srt` VẪN bị từ chối: `Lệnh được chạy … chạy file nằm trong thư mục được ghi — Dev có thể sửa file đó trước khi chạy`. Nguyên nhân: `planfile::validate_grants` (vòng lặp cuối, ~dòng 845-870) kiểm MỌI từ của lệnh so với thư mục được ghi, kể cả đối số dữ liệu. Ý định đúng là chỉ chặn việc THỰC THI một file Dev có thể ghi đè (chương trình hoặc script mà trình thông dịch chạy), không chặn đọc dữ liệu từ thư mục đầu ra.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/planfile.rs` | `validate_grants`, vòng lặp "a command may not run a file the Dev can write itself" | Chỉ xét các từ được THỰC THI: (a) từ đầu tiên (chương trình); (b) nếu từ đầu tiên là trình thông dịch/chạy script (đọc danh sách trình thông dịch đang dùng ở `is_forbidden_program`/phần quy tắc `-c`, ví dụ `python`, `python3`, `node`, `ruby`, `perl`, `php`, `deno`, `bun` — dùng lại đúng danh sách đó, không nhân đôi) thì từ không bắt đầu bằng `-` đầu tiên sau nó (file script). Các đối số còn lại là dữ liệu và KHÔNG bị kiểm theo thư mục ghi. Giữ nguyên thông báo lỗi khi vi phạm đúng nghĩa (script hoặc chương trình nằm trong thư mục ghi). | Cho phép đọc dữ liệu từ thư mục đầu ra |
| 2 | `agent-brain/src/planfile.rs` | `grant_command_error`, thông báo cho `*` | Khi một từ chứa `*` nhưng không phải từ `*` đứng riêng ở cuối, trả: ``*` chỉ được đứng riêng làm từ cuối cùng (ví dụ `python scripts/x.py *`); không viết `*.srt` hay `file*` — hãy liệt kê đúng tên file hoặc dùng `*` đứng riêng``. Không đổi điều kiện từ chối. | Lead tự sửa đúng ở vòng sau |
| 3 | `agent-brain/src/guard.rs` | kiểm tra của hook (không đổi hành vi) | KHÔNG đổi gì ở hook; xác nhận bằng test rằng hook vẫn chặn Dev ghi đè/chạy script nằm trong thư mục ghi: nếu hook hiện chưa chặn việc chạy script nằm trong `write_dirs` thì thêm đúng một kiểm tra trong `bash_grant_check`: chương trình hoặc script (như mục 1) có đường dẫn nằm dưới một thư mục ghi → từ chối `lệnh chạy file trong thư mục được ghi — không được phép`. (Hook không có sẵn danh sách thư mục ghi dạng tương đối: dùng `policy.write_dirs` đã chuẩn hóa dưới gốc.) | Giữ rào chắn thật ở thời điểm chạy, không chỉ ở lúc kiểm plan |
| 4 | `agent-brain/src/planfile.rs` (tests) và `agent-brain/src/guard.rs` (tests) | cuối module test | Test: (a) `python scripts/parse_srt.py projects/x/input/a.srt` với thư mục ghi `projects/x/**` HỢP LỆ; (b) `python projects/x/run.py` với thư mục ghi `projects/x/**` BỊ TỪ CHỐI; (c) `projects/x/tool arg` (chương trình trong thư mục ghi) BỊ TỪ CHỐI; (d) `python -m mod`/`python -c` vẫn bị từ chối như cũ; (e) `python scripts/a.py --out projects/x/o.txt` HỢP LỆ; (f) `subtitles.srt*` bị từ chối với thông báo mới (chứa "đứng riêng"); (g) hook: lệnh khớp mẫu và chạy script ngoài thư mục ghi được phép; lệnh chạy script trong thư mục ghi bị chặn; (h) toàn bộ test cũ về grants/guard giữ nguyên và pass. Thêm test tích hợp nhỏ: dựng `.plan.md` giống plan của dự án `srt` (bốn mục quyền; lệnh `python scripts/create_project.py ganh-nang-tren-vai` và `python scripts/parse_srt.py projects/ganh-nang-tren-vai/input/subtitles.srt`; ghi `.gsd/phases/ganh-nang-tren-vai/**`, `projects/ganh-nang-tren-vai/**`) và khẳng định `validate_plan` trả rỗng. Dùng dữ liệu giả trong thư mục tạm, không đọc dự án thật. | Chốt hành vi |
| 5 | `agent-brain/README.md` | mục quyền chạy của plan (D-49) | Sửa mô tả: quy tắc "không chạy file trong thư mục ghi" chỉ áp cho chương trình và file script được thực thi; đối số dữ liệu trong thư mục ghi được phép; nêu `*` phải đứng riêng ở cuối. | Tài liệu khớp |

## Không được đụng tới
- Danh sách chương trình cấm, danh sách ký tự cấm, quy tắc thư mục nhạy cảm, `..`, đường dẫn tuyệt đối, symlink; hành vi của hook cho `read_dirs`/`write_dirs`/Bash ngoài mục 3. KHÔNG nới bất kỳ rào chắn nào ngoài đúng quy tắc ở mục 1.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dự án `srt` thật và file `.plan.md` của nó (không đọc, không sửa trong test), `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --lib planfile 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --lib guard 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --test cli_dev --test cli_lead --test guard_git 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"      # toàn bộ: không lỗi
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(planfile|guard)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -3
```
Acceptance:
1. Plan có `python scripts/parse_srt.py projects/<slug>/input/subtitles.srt` với thư mục ghi `projects/<slug>/**` hợp lệ; plan giống plan `srt` qua `validate_plan` không lỗi.
2. Chạy chương trình/script nằm trong thư mục ghi vẫn bị từ chối (cả lúc kiểm plan lẫn ở hook).
3. `*.srt` / `file*` bị từ chối với thông báo hướng dẫn mới.
4. Mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `fix(agent-brain): only the executed file of a granted command may not live in a write dir` (code, test), `docs: grants rule clarification` (README). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify, phần chưa kiểm.
