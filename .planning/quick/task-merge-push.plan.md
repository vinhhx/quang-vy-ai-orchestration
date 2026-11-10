# Plan task-merge-push: nút "Merge & push" cho task đã QA đạt (2026-11-10)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm. Plan này chạy SONG SONG với Phase 7 (Journal Vault): xem mục "Không đụng" để tránh xung đột.

## Bối cảnh
- Sau khi QA đạt, người dùng phải tự checkout nhánh gốc, merge nhánh task và push. Các dự án dùng chung một working tree (không bật chế độ worktree).
- Đã có sẵn, dùng lại:
  - `workspace::merge_task` (`src/workspace.rs:300`): `git merge --no-ff`, xử lý xung đột ở file giao thức, huỷ merge nếu xung đột file khác, **không push**, đòi repo đang ở nhánh gốc và sạch (`merge_refusal`, `:267`). `cmd_merge` (`:445`) là lệnh `agent-brain task merge <id>`.
  - `workspace::merge_base_of` (`:161`): nhánh gốc = `task.base_branch`, nếu rỗng thì `project.workspace.base_branch`.
  - `gitops::is_merged_into` (`src/gitops.rs:445`), `gitops::git`, `current_branch`, `changed_paths`, `branch_exists`, `is_ancestor`, `head_sha`.
  - Mẫu một control có hộp xác nhận: `cleanup_worktree` (`src/dashboard/taskstate.rs:825`, route `src/dashboard/api_tasks.rs:568`, danh sách control `:1866`, ánh xạ `:3373`, xử lý `:3804`, hàm `post_cleanup_worktree` `:3995`; JS `assets/dashboard/js/task.js` quanh dòng `1827`, `2533`, `2687-2720`, `2755`, `2794`).
  - Nút Telegram hai lần bấm (xác nhận lần hai): xem `src/telegram/callback.rs` (quota re-run, duyệt plan Telegram) và tin `QA đạt` ở `src/telegram/render.rs:402`.
  - `human::try_push` (`src/human.rs:46`) push không force; `src/gitops.rs:10` `git_cmd` đặt `GIT_TERMINAL_PROMPT=0`.
- Agent AI bị chặn push bởi `guard`; việc merge/push do chính `agent-brain` làm, không qua model.

## Quyết định của người dùng (2026-11-10)
1. Nhánh gốc = `base_branch` của task/dự án; nếu cả hai trống thì mặc định `dev` (cấu hình được).
2. Vẫn cần **một cú bấm xác nhận** (không tự động hoàn toàn).
3. Giữ các commit của task bằng `git merge --no-ff`, thông điệp merge có mã task để `git log --first-parent` ra một dòng mỗi task.
4. Không cần giữ nhánh task: không push nhánh task, xoá nhánh task cục bộ sau khi merge thành công (`git branch -d`, không `-D`).

## Mục tiêu
1. `agent-brain task merge <id> --push`: chuẩn bị nhánh gốc, merge, push, dọn nhánh task. `task merge <id>` (không cờ) giữ nguyên hành vi cũ.
2. Control `merge_push` trên dashboard (nút `Merge & push` ở màn task, hộp xác nhận một lần bấm) dùng cùng một hàm lõi.
3. Telegram: tin `QA đạt` có nút `Merge & push`; bấm lần một hỏi xác nhận, bấm lần hai mới chạy; kết quả gửi lại thành một tin ngắn.
4. Chạy lại được an toàn sau lỗi giữa chừng (idempotent), không bao giờ force-push, không bao giờ `reset --hard`.

## Phạm vi file (chỉ sửa trong)
- `agent-brain/src/gitops.rs`: chỉ **thêm** hàm mới (xem mục 1), không đổi hành vi hàm cũ.
- `agent-brain/src/workspace.rs`: thêm `merge_push_task` và các hàm phụ; không đổi `merge_task`/`merge_refusal` hiện có (chỉ gọi lại).
- `agent-brain/src/config.rs`: thêm mục `merge` (mục 2).
- `agent-brain/src/main.rs`: thêm cờ `--push` cho `task merge` (**chỉ thêm**, giữ diff nhỏ).
- `agent-brain/src/dashboard/taskstate.rs`, `agent-brain/src/dashboard/api_tasks.rs`: control và route mới.
- `agent-brain/assets/dashboard/screens/task.html`, `agent-brain/assets/dashboard/js/task.js`, và file CSS dựng sẵn nếu lớp Tailwind mới buộc phải dựng lại (`bash scripts/build-css.sh`).
- `agent-brain/src/telegram/render.rs`, `agent-brain/src/telegram/callback.rs` (và file khai báo kiểu nút nếu callback nằm riêng).
- Test mới: `agent-brain/tests/task_merge_push.rs`, thêm test vào `agent-brain/tests/dashboard_actions.rs` và một file test Telegram mới `agent-brain/tests/telegram_merge_push.rs` (theo mẫu `tests/telegram_notify.rs` / `tests/telegram_quota.rs`), test `#[cfg(test)]` trong các file trên.
- `agent-brain/README.md`: **chỉ thêm** một mục mới ở cuối phần lệnh CLI và một đoạn mô tả khoá `merge` (append-only).

## Không đụng
`.planning/STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md`, mọi file `.planning/phases/07-*`, `agent-brain/src/journal/**`, `agent-brain/src/db.rs` (plan này không cần migration: trạng thái "đã merge / đã push" suy ra từ git), `~/.config/agent-brain/**`, `~/.local/share/agent-brain/**`, binary đã cài `~/.local/bin/agent-brain`, `~/.claude-orca`. Không `git add -A`, không `--no-verify`.

## 0. Trước khi sửa
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
git rev-parse HEAD
cargo test --no-fail-fast --offline --locked 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
```
Ghi số nền (passed/failed) vào tóm tắt. Nếu có test đỏ sẵn, ghi tên chúng và không sửa chúng.

## 1. Chạy git có timeout và không tương tác (`src/gitops.rs`)
Thêm (không đụng `git`, `git_cmd`):
- `pub fn git_net(root: &Path, args: &[&str], timeout: Duration) -> BrainResult<String>`: như `git` nhưng đặt thêm `GIT_TERMINAL_PROMPT=0`, `GIT_SSH_COMMAND=ssh -o BatchMode=yes -o ConnectTimeout=20` (chỉ khi biến `GIT_SSH_COMMAND` chưa được người dùng đặt), chạy qua `std::process::Command` với vòng chờ `try_wait` đến `timeout`; quá hạn thì `kill` tiến trình con rồi trả lỗi tiếng Việt `git <lệnh đầu> quá <N>s`. Không dùng shell.
- `pub fn remote_url(root: &Path, name: &str) -> Option<String>`: `git remote get-url <name>`; `None` nếu không có.
- `pub fn rev_parse(root: &Path, rev: &str) -> Option<String>`.
- `pub fn ahead_behind(root: &Path, local: &str, upstream: &str) -> BrainResult<(usize, usize)>`: `git rev-list --left-right --count <local>...<upstream>`.
- `pub fn log_subjects(root: &Path, range: &str, max: usize) -> Vec<String>`.
- Kiểm tên nhánh: `pub fn valid_branch_name(name: &str) -> bool` dùng `git check-ref-format --branch`, từ chối tên rỗng, bắt đầu bằng `-`, chứa khoảng trắng hoặc ký tự điều khiển.
Test (`#[cfg(test)]` hoặc `tests/task_merge_push.rs`): hết giờ thật sự giết tiến trình (dùng một hook `pre-receive` ngủ dài hoặc lệnh `git` giả qua PATH); `valid_branch_name` từ chối `-x`, `a b`, `a..b`, chuỗi rỗng.

## 2. Cấu hình `merge` (`src/config.rs`)
Mục `merge` trong `config.json`, mọi khoá tùy chọn, thiếu = giá trị mặc định (theo cách `orchestrator` được đọc, `src/config.rs:1394`; lỗi kiểu dữ liệu báo tiếng Việt như các khoá khác):
- `merge.default_base_branch`: chuỗi, mặc định `dev`, phải qua `valid_branch_name`.
- `merge.git_timeout_secs`: số nguyên 10..=900, mặc định 120.
- `merge.delete_task_branch`: boolean, mặc định `true`.
Thêm vào `Config` một trường `merge: MergeConfig` (có `Default`). Cập nhật `assets/config.default.json` nếu file đó liệt kê các mục khác (giữ khớp `tests/assets_parity.rs`). Test: mặc định khi thiếu; mỗi khoá sai kiểu hoặc ngoài khoảng bị từ chối.

## 3. Hàm lõi `merge_push_task` (`src/workspace.rs`)
Chữ ký gợi ý: `pub fn merge_push_task(cfg: &Config, project: &Project, root: &Path, task: &TaskRow, busy: Option<String>) -> BrainResult<Result<MergePushOutcome, String>>`; `Err(String)` trong `Result` bên trong là lý do từ chối (người dùng đọc được); `BrainResult` lỗi hệ thống. `MergePushOutcome { base, branch, merge_commit: Option<String>, pushed: bool, branch_deleted: bool, already_done: bool, notes: Vec<String> }` có hàm `message()` trả một chuỗi tiếng Việt ngắn.

Nhánh gốc: `merge_base_of(project, task)` rồi `cfg.merge.default_base_branch`.

**Trạng thái suy ra từ git (không lưu DB):**
- A: nhánh task chưa là tổ tiên của nhánh gốc cục bộ.
- B: đã merge vào nhánh gốc cục bộ nhưng `refs/remotes/origin/<base>` chưa chứa commit đó.
- C: `refs/remotes/origin/<base>` đã chứa nhánh task.

**Điều kiện từ chối (theo thứ tự, trả lý do rồi dừng; chưa đổi gì trong repo):**
1. `task.status` khác `qa_pass` (dùng đúng câu của `merge_refusal`); ngoại lệ: trạng thái C thì trả `already_done`.
2. `busy` có giá trị (đang có run hoặc autopilot của dự án): trả câu đó. Người gọi truyền `busy_reason` của dự án (dashboard/Telegram dùng `api_tasks::busy_reason`; CLI dùng cơ chế tương đương trong `autopilot::claim`).
3. Tên nhánh gốc hoặc nhánh task không qua `valid_branch_name`.
4. Working tree có thay đổi chưa commit (`changed_paths` không rỗng): liệt kê tối đa 10 đường dẫn. Ngoại lệ: nếu *chỉ* gồm các file giao thức của task (`protocol_paths::paths(project, &task.slug)`) thì cũng vẫn từ chối, nhưng câu nói rõ "chỉ còn file giao thức chưa commit".
5. Không có nhánh task cục bộ (`branch_exists`) và không ở trạng thái C.
6. Có `origin` (`remote_url`)? Nếu **không có** origin: chạy merge cục bộ (bước d-e dưới đây), bỏ push, `notes` ghi "Repo không có origin, chưa push".

**Các bước (có origin):**
a. `git fetch origin <base>` qua `git_net` (timeout `merge.git_timeout_secs`). Lỗi ⇒ từ chối `Không fetch được origin/<base>: <lỗi>`.
b. Quan hệ nhánh gốc cục bộ với `origin/<base>` (`ahead_behind`): chưa có nhánh gốc cục bộ ⇒ sẽ tạo từ `origin/<base>` ở bước c; cục bộ **ahead** (có commit chưa push) mà không phải trạng thái B hợp lệ ⇒ từ chối, nêu số commit và tối đa 5 tiêu đề (đẩy nhánh gốc sẽ kéo theo chúng); **diverged** ⇒ từ chối. Trạng thái B hợp lệ: mọi commit trong `origin/<base>..<base>` (đi theo `--first-parent`) là merge của chính task này (tiêu đề chứa `[task #<id>]`).
c. Ghi `orig_branch` (`current_branch`). Checkout nhánh gốc: `git checkout <base>` (hoặc `git checkout -b <base> origin/<base>` nếu chưa có). Lỗi ⇒ từ chối (repo vẫn ở `orig_branch`). Nếu cục bộ **behind**: `git merge --ff-only origin/<base>`.
d. Nếu trạng thái A: `git merge --no-ff --no-commit <task.branch>`.
   - Xung đột: tách danh sách file xung đột như `merge_task` (`protocol_paths::paths`): xung đột file giao thức xử lý như `merge_task` (giữ bản của nhánh gốc nếu dự án có `protocol_dir`, ngược lại `git rm -f`); **mọi xung đột file khác** ⇒ `git merge --abort`, `git checkout <orig_branch>`, từ chối kèm danh sách file (tối đa 10). Tách logic xử lý xung đột của `merge_task` thành hàm dùng chung (refactor giữ nguyên hành vi, có test cũ chứng minh) thay vì sao chép.
   - Sau khi merge không xung đột hoặc đã xử lý xong: **loại file giao thức ra khỏi cây kết quả**: với mỗi đường dẫn trong `protocol_paths::paths(...)` mà nhánh gốc **không** theo dõi (`git ls-tree <base> -- <path>` rỗng) mà merge vừa thêm vào index, chạy `git rm -q -f --cached -- <path>` rồi xoá file khỏi working tree nếu còn; nếu nhánh gốc có theo dõi, `git checkout <base> -- <path>`. Lịch sử của nhánh task (kể cả commit có file giao thức) vẫn nằm trong cha thứ hai của merge.
   - `git commit -m <thông điệp>` (xem dưới). Lỗi ⇒ `git merge --abort`, quay về `orig_branch`, từ chối.
e. Nếu trạng thái B hoặc sau khi merge: bước push.
f. Push: `git push origin refs/heads/<base>:refs/heads/<base>` qua `git_net`. **Không** `--force`, không refspec bắt đầu bằng `+`. Lỗi (bị từ chối, không có quyền, mất mạng, quá giờ) ⇒ **giữ nguyên merge cục bộ**, trả `Ok(Ok(outcome))` với `pushed=false` và `notes` ghi lỗi đã cắt gọn và câu "Bấm Merge & push lần nữa để thử push lại" (trạng thái B, chạy lại chỉ push).
g. Sau khi push thành công (hoặc trạng thái C), nếu `merge.delete_task_branch`: `git branch -d <task.branch>` (không `-D`; lỗi thì ghi vào `notes`, không coi là thất bại). Task có `workdir` (worktree) thì không xoá nhánh, ghi chú dùng `Dọn worktree` trước.
h. Repo ở lại nhánh gốc (không quay về `orig_branch`), trừ khi thất bại ở bước c-d (đã quay về `orig_branch`).

**Thông điệp merge** (khớp phong cách lịch sử hiện có của người dùng: `Merge <nhánh> vào <đích>: ...`):
```
Merge <task.branch> vào <base> [task #<id>]: <tiêu đề task, dòng đầu, tối đa 80 ký tự, bỏ ký tự điều khiển>

Dự án: <project.id>
Task: #<id> · QA: đạt
```
Tiêu đề rỗng thì bỏ phần sau dấu hai chấm. Không đưa nội dung plan hay diff vào thông điệp.

**Nhật ký:** mỗi bước in một dòng `term::emit("task <id>: ...")` (hiện trong log của `serve` và terminal CLI): fetch, checkout, merge, loại file giao thức (nếu có), push (kèm sha), xoá nhánh, và mọi từ chối. Không in URL remote có thông tin xác thực.

**Bảo mật/an toàn:** mọi lệnh git dùng mảng tham số, tuyệt đối không shell; tên nhánh đưa vào lệnh sau khi qua `valid_branch_name` và đứng sau `--` khi git cho phép; không bao giờ `--force`, `reset --hard`, `clean`, `branch -D`, `push --delete`.

## 4. CLI (`src/main.rs`, `src/workspace.rs::cmd_merge`)
`agent-brain task merge <TASK_ID> [--push]`. Không có `--push`: hành vi cũ (`merge_task`). Có `--push`: tìm `busy` cho dự án (cùng hàm CLI dùng ở nơi khác), gọi `merge_push_task`, in `outcome.message()`, mã thoát 0 khi `pushed` hoặc `already_done` hoặc (không có origin và đã merge), mã `EXIT_ERROR` hiện có khi bị từ chối hoặc `pushed=false` do lỗi. Cập nhật mô tả lệnh trong `--help` (tiếng Việt).

## 5. Dashboard
- `taskstate.rs`: `build_merge_push(task, busy, git_state) -> ActionView { id: "merge_push", label: "Merge & push", enabled, reason, run_id: None }`. Chỉ dùng kiểm tra **cục bộ, không mạng** (control được tính mỗi lần poll): bật khi `status == "qa_pass"`, không busy, nhánh task còn tồn tại hoặc đã ở trạng thái C. `reason` khi tắt nêu lý do (vd. "Chỉ merge task đã qua QA", "Dự án đang có run chạy"). Khi đã ở trạng thái C hiển thị `Đã merge vào <base> và push` thay vì bật nút. Trạng thái B hiển thị `Đã merge cục bộ, chưa push` và nút vẫn bật (nhãn `Push lại`).
- `api_tasks.rs`: route `POST /api/tasks/{id}/merge-push` (body `{ "rev": ... }` giống `post_cleanup_worktree`), thêm control vào danh sách ở chỗ `cleanup_worktree` (`:1866`) và nhánh xử lý trong `run_task_action` cùng ánh xạ như `:3373`. Xử lý đồng bộ trong `blocking(...)`; kết quả: thành công trả `ok` kèm `message`; từ chối trả `Refused { reason, kind: "merge_refused" }` theo kiểu hiện có (HTTP 409/422 như các control khác); middleware token và header như mọi route ghi. Không chạy được khi `busy_reason` có giá trị.
- `task.html` + `task.js`: nút `Merge & push` đặt cạnh các nút điều khiển task `qa_pass`; bấm mở hộp xác nhận có chữ `Merge nhánh <task.branch> vào <base> rồi push lên origin?` (một nút `Merge & push`, một nút `Hủy`), theo mẫu hộp của `cleanup_worktree`/`reopen`: tiếng Việt, hỗ trợ bàn phím (Esc đóng, focus quay về nút mở), Alpine CSP (không biểu thức inline), lớp Tailwind chỉ dùng những lớp đã có trong CSS dựng sẵn hoặc dựng lại bằng `bash scripts/build-css.sh`. Sau khi xong hiện `message` trong khu vực thông báo hiện có và tải lại trạng thái.

## 6. Telegram
- `render.rs`: tin `QA đạt` (`EventKind::QaPass`, `:402`) thêm một nút bấm `Merge & push` khi control `merge_push` đang bật (theo cách các tin khác đính nút: xem `callback.rs` và nơi dựng bàn phím).
- `callback.rs`: bấm lần một ⇒ sửa tin hoặc gửi tin hỏi `Merge task <id> (<nhánh>) vào <base> và push?` với hai nút `Xác nhận` và `Hủy`; chỉ lần bấm `Xác nhận` mới chạy `run_task_action(..., "merge_push", ...)` (cùng hàm với dashboard), rồi gửi kết quả (`message`) thành một tin. Dùng đúng cơ chế chống bấm trùng, chống tin cũ (`rev`), chỉ chat đã ghép nối, giới hạn độ dài 4096 ký tự như các nút hiện có. `Hủy` không đổi gì.
- Kết quả thất bại/từ chối gửi nguyên lý do ngắn gọn (không gửi đường dẫn tuyệt đối, URL remote, hay token).

## 7. Test
`tests/task_merge_push.rs` dùng git thật với **remote bare cục bộ** làm `origin` và các repo tạm (mẫu `tests/human_engine.rs:21-40`: `Sandbox`, `bare_remote`). Cần có tối thiểu:
1. Đường vui: nhánh gốc `dev` có sẵn, nhánh task có 2 commit, repo đang ở nhánh task, sạch ⇒ sau khi chạy: nhánh gốc có commit merge dạng `Merge <nhánh> vào dev [task #N]: ...`, `origin/dev` chứa nó (`git rev-parse dev == git --git-dir remote rev-parse dev`), nhánh task cục bộ đã bị xoá, repo đang ở `dev`, `git log --first-parent dev` có đúng một dòng cho task.
2. Nhánh gốc cục bộ **behind** `origin/dev` (người khác đã push): fast-forward rồi merge, push thành công.
3. Xung đột ở file không phải giao thức: từ chối, `merge --abort` đã chạy, repo quay về nhánh task ban đầu, `dev` cục bộ và `origin/dev` không đổi, không có commit nào mới.
4. Nhánh gốc cục bộ **ahead** với commit không thuộc task: từ chối nêu số commit, không push gì.
5. Push bị từ chối (hook `pre-receive` thoát 1 trong remote bare): kết quả `pushed=false`, merge cục bộ còn đó, nhánh task chưa bị xoá; sau khi gỡ hook chạy lại ⇒ chỉ push (không có commit merge thứ hai), rồi xoá nhánh.
6. Working tree bẩn ⇒ từ chối, liệt kê file. Trạng thái khác `qa_pass` ⇒ từ chối. Đang busy ⇒ từ chối với đúng câu busy.
7. Không có `origin`: merge cục bộ thành công, `pushed=false`, ghi chú "chưa push", mã thoát 0.
8. Task không có `base_branch` và dự án không khai ⇒ dùng `merge.default_base_branch` (`dev`); đổi cấu hình sang `main` thì dùng `main`.
9. Đã merge và đã push từ trước ⇒ `already_done`, không tạo commit, vẫn dọn nhánh task nếu còn.
10. File giao thức: nhánh task có `.plan.md` và `.qa-report.md` ở gốc, nhánh gốc không theo dõi chúng ⇒ sau merge cây của `dev` không có hai file đó, nhưng `git log <nhánh-task>` (cha thứ hai) vẫn có commit chứa chúng. Biến thể: dự án có `protocol_dir`.
11. Không bao giờ force: kiểm bằng cách bao bọc `git` giả qua PATH ghi lại mọi dòng lệnh rồi khẳng định không có `--force`, `-f`, `+refs`, `reset`, `branch -D`, `clean`.
12. Tên nhánh độc hại (`-x`, `a b`) bị từ chối trước khi chạy bất kỳ lệnh git nào.
Dashboard (`tests/dashboard_actions.rs`): control `merge_push` bật/tắt đúng theo trạng thái; `POST /api/tasks/{id}/merge-push` thành công, bị từ chối khi busy, 401 khi thiếu token, `Stale` khi `rev` cũ.
Telegram (`tests/telegram_merge_push.rs`, dùng fake Telegram `tests/fakes/fake_telegram.py` qua `common::repo_root()`): tin QA đạt có nút; bấm lần một chỉ hỏi xác nhận, không chạy git; bấm `Xác nhận` chạy một lần; bấm lại (tin cũ) không chạy lần hai; chat lạ không chạy được; `Hủy` không đổi gì.

## 8. README (append-only)
Thêm vào `agent-brain/README.md`: một dòng bảng CLI cho `task merge --push`, và mục `### Merge & push sau QA đạt` (khoảng 15 dòng, tiếng Việt): điều kiện, các bước, trạng thái chạy lại an toàn, khoá `merge` trong `config.json`, nhắc "agent-brain không bao giờ force-push và chỉ xoá nhánh đã merge".

## Kiểm tra (bắt buộc chạy, dán kết quả vào tóm tắt)
```bash
cd /mnt/data/IDE/QuangVy-Orchestration
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test task_merge_push
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test dashboard_actions
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --test telegram_merge_push
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --lib
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
node scripts/dashboard-js-check.mjs
```
Mỗi lệnh phải thành công (`failed 0`, mã thoát 0). Tổng test không được giảm so với số nền ở mục 0 và `failed` không tăng so với nền.

## Commit
Mỗi mục lớn một commit riêng (thông điệp tiếng Anh, tiền tố `feat(agent-brain):` hoặc `test(agent-brain):`), chỉ `git add` các file đã nêu, không `git add -A`, không `--no-verify`. Kết thúc thông điệp bằng dòng đồng tác giả mà môi trường yêu cầu.

## Tiêu chí nghiệm thu
- `agent-brain task merge <id> --push` làm đúng các bước 3.a–3.h trên các kịch bản test ở mục 7, bằng git thật.
- Không có đường mã nào dùng `--force`, `reset --hard`, `branch -D`, `clean`, hay shell.
- Nút `Merge & push` hiện đúng lúc ở dashboard và Telegram, cần xác nhận, chạy đúng một lần, và nội dung tin/log không lộ đường dẫn tuyệt đối, URL remote hay token.
- `task merge <id>` (không cờ) và `cleanup_worktree` hoạt động như trước (test cũ vẫn xanh).
- Không file nào ngoài "Phạm vi file" bị đổi (`git diff --name-only <HEAD ban đầu>..HEAD`).

## Tóm tắt phải ghi
File đã đổi, danh sách commit, kết quả các lệnh ở mục "Kiểm tra" (số test trước và sau), mọi chỗ lệch khỏi plan kèm lý do, và các điểm còn mở. Ghi rõ nếu một test không thể viết như mô tả (vd. không mô phỏng được timeout).
