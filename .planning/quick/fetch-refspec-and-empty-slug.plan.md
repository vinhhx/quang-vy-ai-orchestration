# Plan: fetch bằng refspec tường minh, và đường dẫn giao thức khi slug rỗng

Repo: /mnt/data/IDE/QuangVy-Orchestration (nhánh main), crate `agent-brain/`. Chỉ backend Rust. Làm trong worktree của bạn.
Hai việc độc lập, mỗi việc một commit. Cả hai là sửa lỗi nhỏ; không đổi giao diện, không đổi API.

## Việc 1: `task merge --push` báo "Không thấy origin/<base> sau khi fetch"

### Lỗi
`src/workspace.rs` (~dòng 868-878) chạy `git fetch origin refs/heads/<base>` rồi đọc `refs/remotes/origin/<base>`:
```rust
let base_ref = format!("refs/heads/{base}");
crate::gitops::git_net(root, &["fetch", "origin", &base_ref], timeout)
let remote_ref = format!("refs/remotes/origin/{base}");
if crate::gitops::rev_parse(root, &remote_ref).is_none() { refuse("Không thấy origin/{base} sau khi fetch.") }
```
Fetch với refspec trần chỉ cập nhật `refs/remotes/origin/<base>` khi remote có `remote.origin.fetch` trong config. Repo
`automation-test-e2e` không có dòng đó (clone hoặc `git remote add` kiểu không chuẩn; chỉ remote `upstream` có), nên ref theo dõi không được tạo
và merge luôn bị từ chối dù `origin/main` có thật trên remote. Tên nhánh bất kỳ (main, master, ...) đều bị như nhau.

### Sửa
- Trong `src/workspace.rs`: fetch bằng refspec tường minh `+refs/heads/{base}:refs/remotes/origin/{base}` (đích là ref theo dõi,
  dấu `+` chỉ áp lên ref theo dõi cục bộ, không đụng nhánh cục bộ). `base` đã được kiểm tra tên nhánh trước đó (`validate_base_branch`); xác nhận trước khi
  ghép chuỗi và không để `base` chứa ký tự làm hỏng refspec.
- Tìm mọi chỗ khác trong `src/` cùng cách dùng (`grep -rn 'refs/remotes/origin' src` và `grep -rn '"fetch"' src`): `src/human.rs` ~dòng 352-358 fetch `refs/heads/<branch>`
  nhưng đọc `FETCH_HEAD` nên không bị lỗi này; kiểm tra comment ở đó (nói git cập nhật `refs/remotes/origin/<branch>` sau fetch, chỉ đúng khi có refspec) và sửa
  comment cho đúng, KHÔNG đổi logic nếu nó chỉ dùng `FETCH_HEAD`. Nếu có chỗ nào khác đọc `refs/remotes/origin/...` sau fetch trần, sửa tương tự.
- Thông báo lỗi giữ nguyên (tiếng Việt, đã `scrub` URL).

### Test
Thêm test (cùng kiểu các test merge/push hiện có, xem `tests/` có `merge`, `e2e_findings_fixes_d2_worktree`, `task_merge`): dựng repo gốc bare tạm làm `origin`,
repo làm việc có remote `origin` nhưng `git config --unset-all remote.origin.fetch`, nhánh `main` + một nhánh `task/x` đã QA, rồi chạy luồng merge với push:
phải thành công, `origin/main` có merge commit, nhánh task bị xóa. Lặp với nhánh gốc tên `master`. Giữ test hiện có (có refspec chuẩn) vẫn pass.

## Việc 2: đường dẫn giao thức `plan-/` khi slug rỗng

### Lỗi
`protocol_paths::paths_in(protocol_dir, slug)` thay `{slug}` bằng slug. Mặc định mới là `.gsd/quick/plan-{slug}` hoặc `.planning/quick/plan-{slug}`
(`DEFAULT_DIR_GSD`, `DEFAULT_DIR_PLANNING`). Khi slug rỗng (nhánh đang checkout không phải `task/<slug>`: `protocol_paths::for_branch` / `current`,
được `src/roles/{dev,qa,review}.rs` gọi) kết quả thành `.gsd/quick/plan-/.plan.md`, một thư mục `plan-` vô nghĩa. Với mẫu cũ `.brain/{slug}` thì phần rỗng bị bỏ gọn.

### Sửa
- Trong `paths_in`: khi slug rỗng, **bỏ cả đoạn đường dẫn có chứa `{slug}`** (không chỉ bỏ biến), nên `.gsd/quick/plan-{slug}` → `.gsd/quick`, `.brain/{slug}` → `.brain`
  (giữ đúng hành vi cũ cho mẫu chỉ có `{slug}`), `{slug}` một mình → thư mục gốc. Slug khác rỗng: không đổi.
- Xem xét đường chính: `src/roles/{dev,qa,review}.rs` đã có task từ DB (`tasks::current_task_in(...)`, dùng `task.slug`) trước hay sau dòng `protocol_paths::current(...)`.
  Nếu task đã biết thì dùng `protocol_paths::paths(&proj, &task.slug)` thay vì suy slug từ tên nhánh (nguồn đúng hơn, không phụ thuộc nhánh đang checkout);
  nếu việc đó cần đổi thứ tự lớn hoặc lệch phạm vi, KHÔNG làm, chỉ ghi lại trong báo cáo. Phần sửa `paths_in` ở trên là bắt buộc, phần này là tùy chọn.
- Thông báo "Không tìm thấy <plan> ở gốc dự án" (`src/roles/qa.rs` ~dòng 181, `dev.rs` ~dòng 379, `review.rs` ~dòng 109): khi nhánh hiện tại không phải `task/...`
  thêm một câu gợi ý ngắn "Đang ở nhánh <tên>, không phải nhánh task" (tiếng Việt, một câu). Không đổi mã thoát.

### Test
Trong `src/protocol_paths.rs` (module test): `paths_in(".gsd/quick/plan-{slug}", "")`, `paths_in(".brain/{slug}", "")`, `paths_in("{slug}", "")`, `paths_in("docs/x-{slug}/y", "")`
(đoạn có `{slug}` ở giữa bị bỏ → `docs/y`... kiểm tra đúng ý đồ và ghi rõ trong test), cùng với slug thật cho cùng các mẫu. Test cũ (`.brain/{slug}` rỗng → `.brain`) vẫn pass.
Nếu bạn đổi roles sang dùng `task.slug`, thêm một test tích hợp cho dev/qa trên repo ở nhánh khác `task/...` mà vẫn tìm đúng file plan.

## Phạm vi file
`agent-brain/src/workspace.rs`, `agent-brain/src/human.rs` (chỉ comment, trừ khi có chỗ phải sửa logic), `agent-brain/src/protocol_paths.rs`,
`agent-brain/src/roles/{dev,qa,review}.rs` (chỉ thông báo và, nếu chọn làm, nguồn slug), và test tương ứng trong `agent-brain/tests/` hoặc module test.
Không đụng: frontend `assets/`, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` chưa tracked, `~/.claude-orca`.

## Các bước
1. Đọc đủ các đoạn trên, rồi `grep -rn 'refs/remotes/origin' src` và `grep -rn 'protocol_paths::current\|for_branch' src`.
2. Việc 1 + test; commit `fix(agent-brain): fetch the base branch with an explicit refspec so origin/<base> exists`.
3. Việc 2 + test; commit `fix(agent-brain): no empty plan- folder when the task slug is unknown`.
4. Mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Không `git add -A`, không `--no-verify`.

## Tiêu chí nghiệm thu
- Repo thiếu `remote.origin.fetch`: `task merge --push` thành công với nhánh gốc `main` và `master`; không còn thông báo "Không thấy origin/<base> sau khi fetch" khi remote có nhánh đó.
- Remote thật sự không có nhánh gốc: vẫn báo lỗi rõ như trước (không báo thành công giả).
- Slug rỗng không bao giờ cho ra đoạn `plan-` đứng một mình; mẫu `.brain/{slug}` vẫn ra `.brain` như trước.
- Không test cũ bị yếu đi hay bỏ qua.

## Lệnh kiểm tra (trong `agent-brain/`, từ worktree)
- `cargo test --no-fail-fast` (toàn bộ; `telegram_actions::running_run_adds_kill_button` đã biết là hay lỗi do tải máy, chạy lại riêng nếu nó là lỗi duy nhất)
- `cargo build --release` không lỗi, không warning mới
- `bash spikes/lib/scan-secrets.sh` ở root repo

## Báo cáo cuối
File đổi, mã commit, nhánh/worktree, kết quả test, điểm lệch plan (kể cả việc 2 có làm phần tùy chọn hay không và vì sao).
Không merge, không push, không dừng/khởi động lại `agent-brain serve`, không thay `~/.local/bin/agent-brain`.
