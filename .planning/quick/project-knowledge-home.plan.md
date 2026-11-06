# Plan: thư mục tri thức dự án (`.gsd` → `.planning` → `.agent-brain`) và chuyển cấu hình fixbug vào `<thư mục tri thức>/fix-bugs/`

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main` (3d419fb trở lên). Đọc toàn bộ file này trước khi làm.

## Bối cảnh (quyết định người dùng 2026-11-06)
- Mỗi dự án có một **thư mục tri thức** để các agent bám theo tech stack, nghiệp vụ và lưu nhật ký tính năng/bug. Thứ tự tìm: `.gsd/` nếu có; không có thì `.planning/`; không có thì `.agent-brain/`. Cả ba dùng chung cấu trúc kiểu `.gsd` của bizsurvey (`README.md` thứ tự đọc, `STACK.md`, `ARCHITECTURE.md`, `CONVENTIONS.md`, `DOMAIN.md`, `DECISIONS.md`, `QUALITY_GATES.md`, `ROADMAP.md`, `TODO.md`, `phases/`, `fix-bugs/`, `reports/`).
- Plan `fixbug-sources-engine` (đã xong, commit 3d44796..3d419fb) đặt cấu hình ở `<root>/.agent-brain/fixbugs.json`. Đổi thành `<thư mục tri thức>/fix-bugs/fixbugs.json`, đặt cạnh `config.json` của skill `/fix-bugs` cũ trong các dự án `.gsd`. Chưa phát hành nên **không cần** đọc đường dẫn cũ.
- Việc **khởi tạo** thư mục tri thức (hỏi người dùng có tạo `.gsd` không) thuộc plan sau; plan này chỉ tìm và dùng.

## Mục tiêu
1. Module mới `agent-brain/src/knowledge.rs`:
   ```rust
   pub enum HomeKind { Gsd, Planning, AgentBrain }
   pub struct Home { pub kind: HomeKind, pub dir: PathBuf /* tuyệt đối */, pub rel: &'static str /* ".gsd" | ".planning" | ".agent-brain" */ }
   pub fn find(root: &Path) -> Option<Home>;          // thư mục đầu tiên tồn tại theo thứ tự trên; bỏ qua symlink và thứ không phải thư mục
   pub fn find_or_default(root: &Path) -> Home;       // find() hoặc Home{AgentBrain, root/.agent-brain} (chưa tạo)
   pub fn readme(home: &Home) -> Option<PathBuf>;     // .gsd/README.md | .planning/PROJECT.md | .agent-brain/README.md nếu tồn tại
   ```
   Unit test cho từng trường hợp (chỉ `.planning`; `.gsd` + `.planning` → `.gsd`; symlink `.gsd` bị bỏ qua; không có gì → `None` / mặc định `.agent-brain`).
2. Fixbug dùng thư mục tri thức (`src/fixbug/config.rs`, `init.rs`, `mod.rs`, `dashboard/api_fixbug.rs`, `README.md`):
   - Đường dẫn cấu hình = `find_or_default(root).dir/fix-bugs/fixbugs.json`. Bỏ hằng `CONFIG_REL_PATH` cũ; thay bằng hàm `config_path(root) -> PathBuf`. Mọi thông báo (`check`, `NotConfigured`, `init`, API `GET /api/fixbug/{project}`) in đường dẫn **thật** (tương đối với gốc dự án, ví dụ `.gsd/fix-bugs/fixbugs.json`), không còn chữ `.agent-brain/fixbugs.json` cố định.
   - `init`: ghi vào `config_path(root)` (tạo thư mục `fix-bugs/` nếu cần; vẫn không ghi đè file đã có). Nguồn đề xuất từ skill cũ: `<home>/fix-bugs/config.json` (không còn cố định `.gsd/`).
   - API `GET /api/fixbug/{project}` thêm trường `config_path` (tương đối) và `home` (`"gsd"|"planning"|"agent-brain"|null`).
   - `.env.fixbugs` **giữ nguyên** ở gốc dự án (bí mật, đã gitignore ở 8 dự án cũ).
3. Cập nhật mọi test fixbug đang tạo `.agent-brain/fixbugs.json` sang đường dẫn mới, và thêm test: dự án có `.gsd/fix-bugs/config.json` cũ → `init` ghi `.gsd/fix-bugs/fixbugs.json` (không đụng `config.json`); dự án chỉ có `.planning/` → `.planning/fix-bugs/fixbugs.json`; dự án trống → `.agent-brain/fix-bugs/fixbugs.json`.

## Thay đổi (chỉ các file này)
| # | File |
|---|---|
| 1 | `agent-brain/src/knowledge.rs` (mới), `agent-brain/src/lib.rs` |
| 2 | `agent-brain/src/fixbug/{config,init,mod,pull,writeback}.rs` (chỉ chỗ dùng đường dẫn/thông báo) |
| 3 | `agent-brain/src/dashboard/api_fixbug.rs` |
| 4 | `agent-brain/README.md` (dòng mô tả fixbug) |
| 5 | `agent-brain/tests/fixbug_*.rs`, `agent-brain/tests/support/fixbug.rs` |

Không sửa: `Cargo.toml`, `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` thật, repo dự án ngoài, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
grep -rn '\.agent-brain/fixbugs\.json' src tests README.md   # phải rỗng
cargo test --no-fail-fast
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"   # không tăng so với base
git diff --stat <base>..HEAD
```

## Commit
Một commit `feat(agent-brain): project knowledge home and fixbug config under <home>/fix-bugs`. `git add` đúng file, không `-A`, không `--no-verify`, không push, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: file sửa, commit, chỗ lệch plan, clippy base/sau, kết quả kiểm tra.
