# Plan: quyền đọc/ghi cho từng file + Lead biết trình thông dịch có thật trên máy

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Lỗi thực tế (2026-10-29, task `video-srt-idea`, run Dev 14 → `dev:blocked`)
Dev chạy trong skill mode, đọc được `.gsd/phases/_template/**`, nhưng:
1. `guard.jsonl`: `block Read …/.gsd/CHECKLIST_TEMPLATE.md — not in read allowlist` và `block Read …/scripts/create_project.py — not in read allowlist`, dù plan có các dòng `.gsd/CHECKLIST_TEMPLATE.md`, `scripts/create_project.py`, `AGENTS.md` trong "Thư mục được đọc". Nguyên nhân (đã đọc code): `guard.rs::under_grant_dirs` (~dòng 282-288) đòi `resolved != d_real`, nên một mục quyền là MỘT FILE (đường dẫn trùng đúng mục đó) luôn bị từ chối; chỉ file nằm dưới thư mục (`dir/**`) mới qua. Đây là lỗi của đợt skill mode.
2. `python: command not found`: máy chỉ có `/usr/bin/python3` và `/usr/bin/node`, không có `python`. Lời nhắc Lead (`skills::prompt_block`) lại lấy ví dụ `python scripts/ten_script.py *`, nên Lead khai `python …` và Dev chạy thất bại (exit 127).

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/guard.rs` | `under_grant_dirs` | Cho phép `resolved == d_real` khi `d_real` là FILE thường tồn tại (hoặc không tồn tại nhưng mục quyền không kết thúc bằng `/**`/`/` — dùng thông tin có sẵn từ cách `grant_dir_base` suy ra; đọc cách mục `dir/**` và mục file được phân biệt khi `Grants` được chuyển thành `read_dirs`/`write_dirs` trong `roles/dev.rs` và nếu cần chuyển cờ "đây là file" xuống `Policy` qua trường mới `read_files`/`write_files: Vec<PathBuf>` thay vì suy đoán). Mục là thư mục vẫn KHÔNG cho chính đường dẫn thư mục (chỉ file bên dưới); mục trùng gốc dự án vẫn không cấp gì; `.git`, `.planning`, `.env*`, khóa bí mật vẫn luôn bị chặn (`grant_denied_rel` chạy trước). Policy thiếu trường mới vẫn hành vi cũ. | Mục quyền đúng một file hoạt động |
| 2 | `agent-brain/src/roles/dev.rs` | nơi dựng `allowed`/`GuardGrants` từ `Grants` | Với mục quyền là một file: sinh quy tắc `Read(./<file>)` (và `Edit/Write(./<file>)` cho quyền ghi) cùng các dạng `abs_rules`, và đưa file vào `GuardGrants` (mục 1). Mục `dir/**` giữ nguyên. | Claude và hook cùng nhất quán |
| 3 | `agent-brain/src/skills.rs` | `prompt_block`; hàm mới `pub fn available_interpreters() -> (Vec<String>, Vec<String>)` | Tìm trong `PATH` (chỉ kiểm file thường có bit thực thi, KHÔNG chạy chương trình nào) các trình thông dịch/chương trình phổ biến: `python`, `python3`, `node`, `ruby`, `perl`, `php`, `deno`, `bun`, `ffmpeg`; trả (có, không có). `prompt_block` thêm một dòng: `Chương trình có sẵn trên máy này: python3, node, ffmpeg. KHÔNG có: python, ruby … — chỉ khai lệnh với chương trình có sẵn (nếu tài liệu dự án ghi \`python\` mà máy chỉ có \`python3\`, hãy khai \`python3\`).` Đổi ví dụ trong khối từ `python scripts/ten_script.py *` thành dùng chương trình đầu tiên có sẵn (ưu tiên `python3`, rồi `python`, `node`), nếu không có gì thì giữ `python3`. Hàm nhận danh sách thư mục PATH qua tham số/biến để test được. Chỉ gọi `available_interpreters` khi có skill (cùng điều kiện khối hiện tại). | Lead khai đúng lệnh ngay từ đầu |
| 4 | `agent-brain/src/guard.rs` và/hoặc `agent-brain/src/roles/dev.rs` | thông báo Bash bị chặn do exit 127 | KHÔNG đổi logic chạy lệnh. Thêm vào lời nhắc Dev khi có quyền (đoạn đang được thêm vào tin nhắn đầu vào): `Nếu một lệnh báo "command not found", ghi vào mục Vướng mắc tên chương trình có sẵn trên máy: <danh sách từ available_interpreters> và dừng.` | Dev báo gọn và đúng |
| 5 | Tests trong `src/guard.rs`, `src/skills.rs`, `tests/cli_dev.rs` | cuối module/file | Test: (a) quyền đọc một file được phép đọc đúng file đó, không đọc được file anh em cùng thư mục, không đọc được chính thư mục; (b) quyền ghi một file ghi được đúng file đó; (c) mục file là `.env`/`.git/x`/khóa bí mật vẫn bị chặn; (d) `dir/**` vẫn hoạt động như cũ, policy cũ như cũ; (e) `available_interpreters` với PATH giả trong thư mục tạm (tạo file giả `python3` có bit thực thi, không có `python`) → có `python3`, không có `python`; (f) `prompt_block` ghi dòng chương trình có sẵn và ví dụ dùng `python3` khi chỉ có `python3`; không có skill thì `prompt_block` rỗng và không quét PATH; (g) `cli_dev`: plan có mục đọc một file → argv chứa `Read(./file)` và `policy.json` chứa file đó; Dev đọc được. Dữ liệu giả trong thư mục tạm; không đọc dự án `srt` thật. | Chốt hành vi |
| 6 | `agent-brain/README.md` | mục quyền chạy của plan (D-49) | Ghi: mục quyền có thể là một file hoặc `thư-mục/**`; Lead được cho biết chương trình có sẵn trên máy (quét PATH, không chạy gì). | Tài liệu |

## Không được đụng tới
- Danh sách chương trình cấm, ký tự cấm, quy tắc thư mục nhạy cảm, `..`, tuyệt đối, symlink; quy tắc "không chạy file trong thư mục được ghi" (vừa sửa); khớp mẫu lệnh của hook. KHÔNG nới bất kỳ rào chắn nào ngoài đúng mục 1.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dự án `srt` thật, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --lib guard 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --lib skills 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --test cli_dev --test cli_lead --test guard_git 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"      # toàn bộ: không lỗi
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(guard|skills)\.rs|src/roles/dev\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -3
```
Acceptance:
1. Plan khai `.gsd/CHECKLIST_TEMPLATE.md`, `scripts/create_project.py`, `AGENTS.md` trong "Thư mục được đọc": hook cho Dev đọc đúng các file đó và không file nào khác ngoài danh sách.
2. Lead (có skill) nhận dòng "Chương trình có sẵn / KHÔNG có" và ví dụ dùng chương trình có thật.
3. Mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `fix(agent-brain): file entries in plan grants and real interpreters in the Lead block` (code, test), `docs: grants file entries and available programs` (README). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify, phần chưa kiểm.
