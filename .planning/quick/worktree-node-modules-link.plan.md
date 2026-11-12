# Plan: worktree dự án node có `node_modules` sẵn, không làm bẩn cây làm việc và không vi phạm vùng đọc

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (task 53, dự án `automation-test-e2e`, 2026-11-12)
Giao lại task cho lead thì bị chặn: `Working tree có thay đổi chưa commit — dừng, không tự stash: .gsd/scripts/link-node-modules.mjs, node_modules`.

Nguyên nhân (đã xác minh trên worktree thật, trước khi dọn):
1. Worktree mới checkout không có `node_modules` (thư mục bị ignore). Agent dev tự viết `.gsd/scripts/link-node-modules.mjs` rồi tạo symlink `node_modules -> <repo gốc>/node_modules` trong worktree.
2. `.gitignore` của dự án ghi `node_modules/` (có dấu `/`) nên chỉ khớp thư mục, không khớp symlink: git thấy `?? node_modules`. File script cũng là file chưa theo dõi.
3. `gitops.rs::changed_paths` chạy `git status --porcelain=v1 -z --untracked-files=all` và không bỏ qua gì, nên lead (`dashboard/api_tasks.rs::lead_preflight`, `roles/lead.rs`) từ chối.
4. Lần chạy dev trước còn thất bại `protocol_violation`: agy đọc `node_modules/.bin/e2e` qua symlink; đường dẫn thật `/mnt/data/source_codes/code/projects/automation-test-e2e/node_modules/.bin/e2e` không nằm trong vùng đọc (`guard.rs`, "not in read allowlist").

Người dùng đã gỡ tay hai file này và thêm `node_modules` vào `.git/info/exclude` của repo dự án (2026-11-12). Plan này sửa gốc để không phải làm tay lần nào nữa.

## Hiện trạng (đã đọc code)
- `agent-brain/src/workspace.rs`: `add_worktree` (~dòng 50) tạo worktree (hoặc dùng lại thư mục có sẵn đúng nhánh); `create_task_worktree` (~68) gọi nó; `ensure_task_root` (~86) trả `task.workdir` ngay nếu thư mục còn (nhánh trả sớm, KHÔNG qua `add_worktree`), nếu không thì gọi `add_worktree`.
- `agent-brain/src/gitops.rs`: `changed_paths(root)` (~dòng 236); `worktree_add`; `git(root, args)`.
- `agent-brain/src/guard.rs`: quyết định đọc/ghi theo `policy` (`policy.root`, `source_roots`, `read_dirs`, `write_dirs`...); từ chối đọc ngoài vùng ở ~dòng 300-310 với lý do `path {} not in read allowlist`. Đường dẫn được `realpath` trước khi so (symlink bị theo).
- `agent-brain/src/projects.rs`: `WorkspaceOpts` (~dòng 80-135: `from_value`, `from_entry`, hằng khoá như `MAX_PARALLEL_KEY`).

## Thay đổi
| # | File | Hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/projects.rs` | `WorkspaceOpts`, `from_value` | Thêm trường `link_node_modules: bool` (mặc định `true`) đọc từ khoá `link_node_modules` của mục dự án trong `projects_map.json`; thêm hằng `LINK_NODE_MODULES_KEY`. Giá trị không phải bool → giữ mặc định. Không thêm giao diện dashboard (sửa file map để tắt); cập nhật mọi chỗ khởi tạo `WorkspaceOpts { .. }` và test cho biên dịch. | Cho phép tắt theo dự án |
| 2 | `agent-brain/src/workspace.rs` | hàm mới `link_node_modules(project_root: &Path, worktree: &Path)` + gọi | Điều kiện nối: `<project_root>/package.json` là file, `<project_root>/node_modules` là thư mục thật (không phải symlink), `<worktree>/node_modules` chưa tồn tại (kể cả symlink hỏng, dùng `symlink_metadata`). Khi đủ: `std::os::unix::fs::symlink(<project_root>/node_modules, <worktree>/node_modules)` (đường dẫn tuyệt đối, đã canonicalize). Idempotent. Lỗi chỉ `crate::term::emit("Cảnh báo: …")`, KHÔNG làm hỏng việc tạo worktree. Đồng thời ghi dòng `node_modules` (một dòng, không `/`) vào `<git common dir>/info/exclude` nếu chưa có (lấy đường dẫn bằng `git rev-parse --git-path info/exclude` chạy trong worktree; tạo thư mục `info` nếu thiếu; chỉ nối thêm, không sửa dòng khác; chỉ khi vừa nối symlink hoặc symlink đã có). Gọi hàm này (nếu `project.workspace.link_node_modules`): trong `add_worktree` sau khi tạo hoặc dùng lại thư mục, và trong nhánh trả sớm của `ensure_task_root` (để worktree có từ trước, như task 53, cũng được nối). `add_worktree`/`create_task_worktree` hiện không nhận `Project`: thêm tham số cờ `link: bool` hoặc `&WorkspaceOpts` và cập nhật mọi nơi gọi (`grep -rn "create_task_worktree\|add_worktree"`). | Gốc của lỗi: agent không còn phải tự nối |
| 3 | `agent-brain/src/gitops.rs` | `changed_paths` | Bỏ khỏi kết quả mọi đường dẫn mà thành phần cuối là `node_modules` VÀ (`root.join(path)`) là symlink (`symlink_metadata().file_type().is_symlink()`), ở mọi độ sâu. Thư mục `node_modules` thật, file khác chưa theo dõi, và mọi thay đổi khác giữ nguyên. | Lưới an toàn cho dự án cũ hoặc repo không có exclude |
| 4 | `agent-brain/src/guard.rs` | quyết định đọc | Cho phép ĐỌC (không ghi) đường dẫn đã `realpath` nằm dưới `realpath(<policy.root>/node_modules)` khi `<policy.root>/node_modules` là symlink. Không mở rộng thêm: không cho phép các thư mục anh em của đích symlink, không cho phép ghi, `Grep`/`Glob` giữ như cũ. Đọc phần `policy` để biết `policy.root` là gì với worktree và đặt kiểm tra cạnh nhánh "in read-only source project" (~dòng 250-270) với `reason: "in linked node_modules"`. | Hết `protocol_violation` khi đọc `node_modules/.bin/...` |
| 5 | `agent-brain/src/workspace.rs`, `agent-brain/src/gitops.rs`, `agent-brain/src/guard.rs`, `agent-brain/src/projects.rs` | test (`#[cfg(test)]`) cạnh code, hoặc `agent-brain/tests/worktree_node_modules.rs` nếu helper tạo repo tạm đã có ở `tests/common` | Dùng thư mục tạm và `git init`, không đụng repo thật. Ca kiểm: (a) dự án có `package.json` + thư mục `node_modules` → `create_task_worktree` tạo symlink đúng đích và `git status --porcelain` trong worktree RỖNG; `<common>/info/exclude` chứa đúng một dòng `node_modules`; (b) gọi lần hai không nhân đôi dòng, không lỗi; (c) không có `package.json` → không nối, không lỗi; (d) có `package.json` nhưng chưa có `node_modules` → không nối, không lỗi; (e) `link_node_modules: false` → không nối; (f) worktree cũ chưa có symlink đi qua `ensure_task_root` → được nối; (g) `changed_paths` bỏ symlink `node_modules` nhưng vẫn báo thư mục `node_modules` thật có file chưa theo dõi và báo file chưa theo dõi khác; (h) guard: đọc `<worktree>/node_modules/.bin/x` (qua symlink) được phép, đọc thư mục anh em của `node_modules` ở repo gốc bị từ chối, ghi vào `node_modules` bị từ chối; (i) `WorkspaceOpts::from_value` đọc `link_node_modules` bool, giá trị sai kiểu giữ `true`. Không để test đụng `~/.local/share/agent-brain`, `~/.config/agent-brain` (dùng biến `XDG_DATA_HOME`/thư mục tạm như test workspace hiện có). | Chốt hành vi |
| 6 | `agent-brain/README.md` | mục workspace / worktree | Thêm: dự án node trong chế độ worktree tự được nối `node_modules` của repo gốc (symlink, có trong `.git/info/exclude`); khoá `link_node_modules: false` để tắt; lưu ý rủi ro: `npm install` trong worktree ghi vào `node_modules` dùng chung của repo gốc; chỉ nối `node_modules` ở gốc dự án, không nối `packages/*/node_modules`. | Tài liệu trung thực |

## Không được đụng tới
- `autopilot/*`, `orchestrator/*`, `dashboard/*` (kể cả giao diện), `telegram/*`, `roles/*` (chỉ đọc; nếu cần đổi chữ ký ở nơi gọi `create_task_worktree`, sửa tối thiểu và ghi vào summary).
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*` (brain.db thật và worktree thật; serve đang chạy), `/mnt/data/source_codes/**` (repo dự án thật: test KHÔNG ghi vào đó), `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- KHÔNG thay binary `agent-brain` đang chạy. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --lib workspace gitops guard projects 2>&1 | grep -E "^test result|FAILED|panicked"   # nếu cargo không nhận nhiều bộ lọc, chạy từng bộ lọc một
cargo test --offline --locked --test worktree_node_modules 2>&1 | grep -E "^test result|FAILED|panicked"          # nếu test nằm trong tests/
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(workspace|gitops|guard|projects)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test flaky đã biết (không phải của việc này; chạy lại riêng và ghi vào summary): `running_run_adds_kill_button` trong `tests/telegram_actions.rs`, `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. Dự án node (có `package.json` và `node_modules` thật) ở chế độ worktree: worktree mới có `node_modules` là symlink, `git status` rỗng, lead/dev không bị chặn vì dirty.
2. Worktree có từ trước (task 53) được nối khi `ensure_task_root` chạy; không cần agent tự viết script.
3. Đọc qua symlink `node_modules` được phép, không mở thêm quyền đọc/ghi nào khác (test từ chối).
4. Dự án không phải node, hoặc `link_node_modules: false`, không đổi hành vi.
5. Mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa (cộng nơi gọi `create_task_worktree` nếu bắt buộc).

## Quy trình
- Hai commit: `feat(agent-brain): link node_modules into node project worktrees and ignore the symlink in dirty checks` (code, test), `docs: worktree node_modules link` (README). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan (nhất là chữ ký thay đổi của `add_worktree`/`create_task_worktree` và cách guard xác định `policy.root`), kết quả từng lệnh verify (số pass/fail), phần chưa kiểm (chưa chạy trên worktree thật của task 53, serve chưa restart).
