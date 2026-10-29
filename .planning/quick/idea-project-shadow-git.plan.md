# Plan: loại dự án `idea` — không cần `.git` trong thư mục dự án (repo git ẩn do agent-brain quản lý)

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Quyết định của người dùng (2026-10-29)
- Dự án dạng ý tưởng/tài liệu không cần git trong thư mục của nó; thư mục có thể nằm ổ đĩa khác. Chỉ dự án `code` dùng git thật.
- Dự án `idea` chạy theo quy trình đã chọn, tới bước nào chạy tới bước đó (kể cả review, QA nếu quy trình có).
- Phần nhiều path (`extra_paths`) là PLAN 2, KHÔNG làm ở plan này.

## Thiết kế (đã đọc code)
Mọi lệnh git của agent-brain đi qua `gitops.rs::git_cmd(root)`. Vai Lead/Dev/QA/review, `human.rs`, `autopilot` đều dựa vào commit plan, `base_commit`, thư mục sạch, `diff_names`, nên KHÔNG viết lại từng vai. Thay vào đó, dự án `idea` dùng một **repo git ẩn (shadow)** có `GIT_DIR` nằm ngoài thư mục dự án và `GIT_WORK_TREE` = thư mục dự án:
- Shadow gitdir: `<config::data_dir()>/shadow/<fnv1a64 hex của đường dẫn canonical>.git`.
- `git_cmd(root)`: nếu `root/.git` KHÔNG tồn tại và thư mục shadow của `root` tồn tại thì đặt `GIT_DIR` và `GIT_WORK_TREE=root`. Nếu `root/.git` tồn tại thì dùng nó như bình thường (ưu tiên git thật).
- Nhờ vậy `is_repo_root`, `current_branch`, `commit_paths`, nhánh `task/...` hoạt động y như dự án code, và thư mục người dùng không có `.git`.
- Lưu ý ngữ nghĩa: đổi nhánh trong shadow đổi file thật trong thư mục (giống git thường). Task chạy trên nhánh riêng như dự án code.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/gitops.rs` | cạnh `git_cmd` | Thêm `pub fn shadow_dir(root:&Path)->PathBuf` (canonicalize `root`, băm FNV-1a 64-bit viết tay (ổn định giữa phiên bản Rust, không thêm dependency), tên `<16 hex>.git` dưới `config::data_dir().join("shadow")`). Sửa `git_cmd`: nếu `!root.join(".git").exists()` và `shadow_dir(root).is_dir()` thì `cmd.env("GIT_DIR", shadow).env("GIT_WORK_TREE", root)`. Hành vi với repo thật không đổi. | Một điểm duy nhất bật shadow |
| 2 | `agent-brain/src/gitops.rs` | hàm mới `pub fn ensure_shadow(root:&Path)->BrainResult<()>` | (a) nếu `root/.git` tồn tại hoặc shadow đã có: `Ok`. (b) đếm file qua duyệt thư mục (bỏ qua thư mục `.git`, `node_modules`, `target`, symlink không theo): nếu > 20000 file hoặc > 500 MiB thì lỗi tiếng Việt `Thư mục ý tưởng '<path>' quá lớn để theo dõi (>20000 file hoặc >500 MiB) — tách phần tài liệu nhỏ ra hoặc dùng git thật (git init).`; (c) tạo shadow: `git --git-dir=<shadow> init -q`, rồi `config user.name agent-brain`, `config user.email agent-brain@localhost` (chỉ trong shadow); ghi `<shadow>/info/exclude` với các dòng `.env`, `.env.*`, `*.pem`, `*.key`, `id_rsa*`, `node_modules/`, `target/`; (d) commit gốc: `add -A` rồi `commit --allow-empty -m "agent-brain: snapshot ban dau"` qua `git_cmd`; nếu bước (c)/(d) lỗi thì xóa thư mục shadow vừa tạo rồi trả lỗi (không để shadow nửa vời). Tên nhánh mặc định đặt `main` (`init -b main` hoặc `symbolic-ref`). | Tạo repo ẩn có giới hạn và an toàn |
| 3 | `agent-brain/src/projects.rs` | `require_runnable` (~dòng 230) | Giữ nguyên kiểm tra `is_repo_root` cho mọi loại, nhưng trước kiểm tra đó: `if project.kind == "idea" { crate::gitops::ensure_shadow(&proj_path)?; }`. Dự án `code` không có git vẫn bị từ chối như cũ; `media` giữ nguyên. | Dự án idea chạy được |
| 4 | `agent-brain/src/projects.rs` + nơi phân tích tham số `--type` của `projects add` (tìm bằng `grep -rn "\"media\"" src/main.rs src/app.rs src/projects.rs`) | `add`, `new_entry_from_projects`, nhắc lệnh trợ giúp | Cho `--type idea` ngoài `code|media`; `new_entry_from_projects` đặt `repo: null` cho `idea` (đã đúng vì chỉ `code` lấy remote); thông báo lỗi liệt kê đủ ba loại. Không tự suy `idea` từ thư mục không có git (giữ `media`). | Đăng ký dự án idea |
| 5 | `agent-brain/src/` và `agent-brain/assets/dashboard/` | chỗ nào phân biệt `kind` ("code"/"media") ngoài `api_board.rs:94,208` và `telegram/intake.rs:323` | `idea` đối xử như `code` ở: danh sách dự án, tạo task, `/task` Telegram (đã đúng vì chỉ loại `media`), bảng Kanban. Kiểm bằng `grep -rn "media" src assets/dashboard/js assets/dashboard/screens`; chỉ sửa nơi `idea` bị xử như `media` hoặc báo "không hợp lệ". Hiển thị nhãn loại `idea` ở màn Dự án nếu màn đó đang hiện nhãn loại; không thêm class CSS mới, không build lại CSS. | Không để idea bị đối xử sai |
| 6 | `agent-brain/src/gitops.rs` (module `tests`) | cuối file | Test (dùng thư mục tạm, `AGENT_BRAIN_DATA_DIR`/biến môi trường mà `config::data_dir()` hỗ trợ để shadow nằm trong thư mục tạm; đọc `config.rs:940` để biết biến đúng): (a) thư mục không có `.git`: `ensure_shadow` thành công, thư mục KHÔNG có `.git`, shadow tồn tại, `is_repo_root` đúng, `head_sha` ra SHA; (b) `commit_paths` một file mới rồi `diff_names` giữa hai SHA ra đúng file; (c) tạo nhánh `task/x`, đổi nhánh, `current_branch` đúng; (d) gọi lại `ensure_shadow` là no-op (không tạo commit gốc thứ hai); (e) thư mục đã có `.git` thật: không tạo shadow, git dùng `.git` đó; (f) vượt giới hạn: lỗi đúng thông báo và KHÔNG để lại thư mục shadow; (g) file `.env` không bị `add -A` đưa vào commit gốc; (h) hai thư mục cùng tên khác đường dẫn có shadow khác nhau. | Chốt hành vi |
| 7 | `agent-brain/src/projects.rs` (tests) và `agent-brain/tests/` (chọn file có sẵn fake claude, ví dụ `cli_lead.rs`) | cuối file | Unit: `require_runnable` idea + thư mục không git → `Ok`; code + thư mục không git → lỗi "không phải repo git" như cũ; media không đổi. Tích hợp: dự án `idea` (thư mục tạm không `.git`, `type: "idea"`) chạy Lead bằng claude giả → exit 0, `.plan.md` được commit trong shadow, thư mục vẫn không có `.git`, `tasks.base_commit` đặt. Nếu harness cần nhiều công dựng, làm tối thiểu và ghi vào summary. | Kiểm cả luồng |
| 8 | `agent-brain/README.md` | bảng lệnh `projects add` (~dòng 72) và mục dự án/loại dự án | Ghi: loại `idea` (không cần `.git`; repo ẩn tại `~/.local/share/agent-brain/shadow/`; thư mục không bị thêm `.git`; chạy theo quy trình đã chọn; giới hạn 20000 file / 500 MiB; `.env`, `*.pem`, `*.key`, `id_rsa*` loại khỏi theo dõi; không có remote nên không đẩy lên được; đổi nhánh đổi file thật); ghi rõ nhiều path chưa hỗ trợ. | Tài liệu |
| 9 | `.planning/phases/06-multi-engine-media-branch/06-CONTEXT.md` | sau D-46 | Thêm `- **D-47 (user 2026-10-29):** Project type \`idea\` needs no \`.git\` in its folder; agent-brain keeps a hidden shadow git repo (GIT_DIR outside the folder) so every role and workflow runs unchanged. Multiple paths per project is deferred to a second plan.` | Ghi quyết định |

## Không được đụng tới
- Logic các vai (`roles/*.rs`), `autopilot/`, `human.rs`, `dashboard/api_tasks.rs` — chỉ sửa nếu bước 5 bắt buộc, và khi đó ghi rõ vào summary.
- Hành vi git của dự án `code` và `media`.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` thật (test dùng thư mục tạm), `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- `git add -A` trong repo của dự án này, `--no-verify`: cấm (lưu ý: `add -A` trong shadow của dự án idea là có chủ đích, chỉ trong code `ensure_shadow`). Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --lib gitops projects 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"      # toàn bộ: không lỗi
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(gitops|projects)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -3
```
Acceptance:
1. Thư mục không `.git`, `type: "idea"`: `require_runnable` Ok; Lead tạo nhánh task, commit plan; Dev commit; `diff_names` đúng; thư mục dự án KHÔNG xuất hiện `.git`.
2. Dự án `code` không git vẫn bị từ chối; dự án có `.git` thật không bị tạo shadow.
3. Thư mục quá lớn bị từ chối bằng thông báo tiếng Việt và không để lại shadow nửa vời.
4. `.env`, `*.pem`, `*.key` không vào commit gốc.
5. Toàn bộ test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): idea project type with hidden shadow git` (code, test), `docs: idea project type and D-47` (README, CONTEXT). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là bước 5 và bước 7), kết quả từng lệnh verify, phần chưa kiểm (chưa chạy thật qua Telegram/dashboard).
