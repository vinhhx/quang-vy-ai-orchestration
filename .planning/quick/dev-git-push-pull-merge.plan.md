# Plan: Dev được push (trừ main/master), pull, merge, checkout/switch; vẫn cấm reset và xoá

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm. Plan trước đó (đã xong): `.planning/quick/agent-git-permissions.plan.md` và `-fix1.plan.md`.

## Yêu cầu của người dùng
Mở thêm `push` (không push lên `main`/`master`), `pull`, `merge`, `checkout`. Vẫn cấm reset và xoá.

## Quyết định thiết kế (đã chốt, làm đúng như sau)
1. **Chỉ Dev** được dùng các lệnh mới, và chỉ khi plan đã duyệt khai trong `## Lệnh được chạy`, ví dụ `git push origin *`, `git pull *`, `git merge *`, `git checkout *`, `git switch *`. Lớp allowedTools của Dev đã tự thêm `Bash(<lệnh>)` từ plan (`src/roles/dev.rs` dòng ~421).
   - Lead, QA, Review KHÔNG được thêm, vì Lead chỉ được sửa `.plan.md`.
   - Không đổi `LEAD_ALLOWED_TOOLS`, `QA_ALLOWED_TOOLS`, `REVIEW_ALLOWED_BASE`.
2. **Merge và pull bắt buộc `--no-ff`; pull bắt buộc thêm `--no-rebase`.**
   - Lý do: hệ thống phải phân biệt được file Dev sửa với file được kéo về từ nhánh khác.
   - Fast-forward hay rebase sẽ đưa commit của người khác lên đường first-parent của nhánh task, khiến mọi file đó bị coi là Dev sửa ngoài plan.
3. **Dev phải kết thúc ở đúng nhánh task.** Dev được checkout hay switch sang nhánh khác trong lúc chạy, nhưng phải quay lại nhánh task trước khi xong.
4. **Không merge hay pull khi đang đứng trên `main`/`master`**, để không làm bẩn main ở máy.
5. **Vẫn cấm:**
   - `reset`, `rebase`, `restore`, `clean`, `stash`, `revert`, `cherry-pick`.
   - `branch -d/-D`, `tag -d`, `push --delete` hay refspec `:x`.
   - `push --force`, `--force-with-lease`, `--mirror`, `--all`, `--tags`.
   - `checkout -f/-B/--orphan`.
   - `checkout -- <file>` hay `checkout <file>`: lệnh này huỷ thay đổi của file, cũng là một kiểu reset.
6. Danh sách nhánh được bảo vệ là hằng `PROTECTED_BRANCHES = ["main", "master"]` trong `guard.rs`, so khớp không phân biệt hoa thường.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/guard.rs` | Mục A, B |
| 2 | `agent-brain/src/planfile.rs` | Mục B (chế độ kiểm tra lúc khai quyền) |
| 3 | `agent-brain/src/gitops.rs` | Mục C, chỉ thêm hàm |
| 4 | `agent-brain/src/roles/dev.rs` | Mục C, D |
| 5 | `agent-brain/assets/prompts/lead.md` | Mục E |
| 6 | `brain/prompts/lead.md` | Sinh lại từ #5 bằng script: bỏ dòng bắt đầu `- Chạy mỗi lệnh git riêng, trên một dòng`. Không gõ tay. |
| 7 | Test: `agent-brain/tests/guard_git.rs`, `agent-brain/tests/cli_dev.rs`, test unit trong `guard.rs`, `planfile.rs`, `gitops.rs` | Mục Test |

### A. `src/guard.rs`: luật cho push, pull, merge, checkout, switch
Thêm `push`, `pull`, `merge`, `checkout`, `switch` vào danh sách subcommand được guard xét (không đưa vào `GIT_READ_SUBCOMMANDS`, không đưa vào `GIT_READ_ONLY_TOOLS`). Mỗi lệnh có một hàm `check_*` riêng, viết theo kiểu `check_fetch`/`check_branch` đã có: **danh sách cho phép** tùy chọn, mọi tùy chọn khác bị từ chối.

Dùng chung một hàm kiểm tra tên remote/nhánh, tách từ phần kiểm tra positional của `check_fetch`:
- Chỉ gồm ký tự `[A-Za-z0-9._/-]`.
- Không bắt đầu bằng `-`, `/` hay `.`.
- Không chứa `..`, `:`, `+`, `@{`.

**A1. `git push`:**
- Long option chỉ được dùng: `set-upstream`, `dry-run`, `quiet`, `verbose`, `porcelain`, `atomic`.
- Cụm ngắn chỉ gồm `u n q v`.
- Bắt buộc có đúng 1 remote (positional đầu tiên, theo luật tên remote ở trên) và **ít nhất 1 refspec**. Từ chối `git push` trần và `git push origin` không có refspec.
- Mỗi refspec:
  - Không bắt đầu bằng `+`.
  - Có `:` thì tách `src:dst`. `src` rỗng (xoá nhánh từ xa) thì từ chối.
  - Không có `:` thì `dst = src`.
  - `src` và `dst` đi qua luật tên nhánh, riêng `:` được phép đúng một lần làm dấu phân tách.
  - `dst`, sau khi bỏ tiền tố `refs/heads/`, nếu nằm trong `PROTECTED_BRANCHES` (không phân biệt hoa thường) thì từ chối với câu `không được push lên nhánh <dst> (main/master được bảo vệ)`.
  - `src == "HEAD"` mà không có `:dst` thì từ chối với câu `ghi rõ tên nhánh đích, ví dụ git push origin HEAD:<nhánh>`.
  - Bắt đầu bằng `refs/` mà không phải `refs/heads/` (ví dụ `refs/tags/`) thì từ chối.

**A2. `git pull`:**
- Long option chỉ được dùng: `no-rebase`, `no-ff`, `no-edit`, `quiet`, `verbose`, `prune`, `all`, `no-tags`, `stat`, `no-stat`.
- Cụm ngắn chỉ gồm `q v p`.
- **Bắt buộc có cả `--no-rebase` và `--no-ff`**. Thiếu thì từ chối với câu `git pull phải có --no-rebase --no-ff (ví dụ git pull --no-rebase --no-ff origin <nhánh>)`.
- Positional: tối đa 1 remote và các tên nhánh, theo luật tên remote/nhánh ở trên (không có `:` nên không có refspec ghi đè).
- Đang đứng trên nhánh bảo vệ thì từ chối (A6).

**A3. `git merge`:**
- Long option chỉ được dùng: `no-ff`, `no-edit`, `no-commit`, `squash`, `stat`, `no-stat`, `quiet`, `verbose`, `abort`, `allow-unrelated-histories`, `log`, `no-log`, `message`.
- Cụm ngắn chỉ gồm `q v n`. Cụm ngắn `-m` có giá trị: xử lý như `commit -m`, phần còn lại của cụm hoặc từ tiếp theo là message.
- Từ chối `-e`/`--edit`, `-F`/`--file`, `-s`/`--strategy`, `-X`/`--strategy-option`, `--continue` (mở editor), `--quit`, `--into-name`, `--rerere-autoupdate`.
- `git merge --abort` đứng một mình được phép.
- Trường hợp khác: **bắt buộc có `--no-ff`** hoặc `--squash`. Positional là tên nhánh hay ref, theo luật tên nhánh, cho phép thêm dạng `origin/<nhánh>`.
- Đang đứng trên nhánh bảo vệ thì từ chối (A6).

**A4. `git checkout`:** chỉ để đổi nhánh hay tạo nhánh, không đụng tới file.
- Dạng được phép:
  - `git checkout <ref>`: đúng 1 positional.
  - `git checkout -b <nhánh-mới> [<ref>]`.
  - `git checkout --detach <ref>`.
  - Tùy chọn phụ được phép: `-q`/`--quiet`, `--track` / `-t` đi kèm `-b`, `--no-track`.
- Từ chối:
  - `--` ở bất kỳ đâu.
  - Hơn 1 positional, trừ dạng `-b <mới> <ref>`.
  - `-f`/`--force`, `-B`, `--orphan`, `-m`/`--merge`, `-p`/`--patch`, `--ours`, `--theirs`, `--conflict`, `--overlay`, `--no-overlay`, `--pathspec-from-file`, `--ignore-other-worktrees`, `--discard-changes`.
- `<ref>` (không áp dụng cho `<nhánh-mới>`):
  - Theo luật tên nhánh, cho phép `origin/x`.
  - Nếu `policy.root.join(<ref>)` tồn tại trên đĩa (file hay thư mục), từ chối với câu `checkout <x> trùng tên file — không được dùng checkout để khôi phục file`.
  - Dùng `git -C <root> rev-parse --verify --quiet <ref>^{commit}` để xác nhận đây là ref. Không phải ref thì từ chối với câu `checkout chỉ dùng để chuyển nhánh — <x> không phải nhánh/commit`.
  - Nếu không có `policy` (kiểm tra lúc khai quyền, mục B) thì bỏ qua hai kiểm tra ổ đĩa và rev-parse.
- `<nhánh-mới>` theo luật tên nhánh, và không được trùng `PROTECTED_BRANCHES`.

**A5. `git switch`:**
- Dạng được phép: `git switch <nhánh>`, `git switch -c <mới> [<ref>]`, `git switch --detach <ref>`, cùng `-q`, `--track`/`-t`, `--no-track`.
- Từ chối `-C`, `--force-create`, `-f`/`--force`, `--discard-changes`, `--orphan`, `-m`/`--merge`, `--conflict`, `--ignore-other-worktrees`.
- Luật tên giống A4. `switch` không nhận path nên không cần kiểm tra file.

**A6. Biết nhánh hiện tại:** guard cần `policy.root` để chạy `git -C <root> symbolic-ref --short -q HEAD`.
- Nhánh hiện tại nằm trong `PROTECTED_BRANCHES` thì `merge`/`pull` bị từ chối với câu `không merge/pull khi đang ở nhánh <x> (main/master được bảo vệ)`.
- Không đọc được nhánh (detached HEAD) thì vẫn cho phép.
- Hiện `bash_command_check(command)` không nhận policy. Thêm hàm `bash_command_check_with(command, root: Option<&Path>)`; `bash_command_check` gọi nó với `None`. `decide_bash` gọi bản có `Some(&policy.root)`.
- Khi `root == None`, A4 và A6 bỏ qua phần cần đọc repo.

**A7.** Cập nhật câu lỗi subcommand không được phép thành: `git <sub> không được phép — chỉ dùng lệnh git chỉ đọc, git fetch, add/commit, hoặc push/pull/merge/checkout/switch khi plan khai; reset, rebase, restore, clean, stash, xoá nhánh/tag luôn bị cấm`.

### B. Kiểm tra lúc khai quyền (`planfile::grant_command_error`)
Hiện tại hàm bỏ `*` cuối rồi gọi `bash_command_check`. Với lệnh mới, cách này từ chối nhầm các mẫu hợp lý như `git push origin *` (thiếu refspec) hay `git merge *` (thiếu `--no-ff`).

Thêm chế độ "mẫu":
- `guard` có `pub fn git_grant_check(words: &[&str]) -> Result<(), String>`. Hàm này chạy cùng các luật cấm (subcommand, tùy chọn, tên/refspec, nhánh bảo vệ), nhưng **bỏ qua các luật bắt buộc phải có**: refspec của push, `--no-ff`/`--no-rebase`, số positional của checkout, message của commit.
- Lý do: phần sau `*` sẽ được guard kiểm tra đầy đủ lúc chạy thật.
- `grant_command_error` dùng hàm này cho lệnh git. Lệnh git không kết thúc bằng `*` vẫn phải qua kiểm tra đầy đủ như cũ.
- Kết quả mong đợi:
  - Hợp lệ: `git push origin *`, `git pull *`, `git pull --no-rebase --no-ff origin *`, `git merge *`, `git merge --no-ff *`, `git checkout *`, `git switch *`, `git commit *`.
  - Không hợp lệ: `git push --force *`, `git push origin main`, `git push origin :x`, `git reset *`, `git rebase *`, `git checkout -- *`, `git checkout -f *`, `git branch -d *`, `git stash *`.

### C. `src/gitops.rs` và `src/roles/dev.rs`: tính file Dev sửa khi có merge
Hiện tại `dev.rs` dùng `diff_names(dev_base, HEAD)` ở dòng ~777 và ~897, và `commit_subjects(dev_base, HEAD)` ở dòng ~887. Sau một merge, tập file này gồm mọi file kéo về từ nhánh khác, nên Dev bị đánh vi phạm.

1. Thêm vào `gitops.rs`: `pub fn dev_changed_names(root, base) -> BrainResult<BTreeSet<String>>`, trả hợp của hai tập:
   - `git log --first-parent --no-merges --format= --name-only <base>..HEAD`: file trong các commit thường của chính nhánh.
   - Với mỗi commit trong `git rev-list --first-parent --merges <base>..HEAD`: `git diff-tree -r --cc --name-only --no-commit-id <merge>`. Đây là các file khác với **mọi** cha, tức là file Dev sửa tay hoặc tự giải xung đột khi merge.
   - Không có merge nào thì kết quả bằng `diff --name-only base..HEAD` như cũ, trừ trường hợp file sửa rồi sửa ngược lại. Viết test cho trường hợp thường.
2. `dev.rs`: dùng `dev_changed_names` thay cho `diff_names` ở chỗ kiểm tra vi phạm (~777) và ở danh sách file cuối (~897). Phần `uncommitted` giữ nguyên.
   - `commit_subjects` (~887): thêm `--first-parent` nếu hàm hiện tại dùng `git log`. Kiểm tra code; nếu phải đổi chữ ký hàm thì thêm hàm mới, không sửa hàm cũ đang được chỗ khác dùng.
3. Không đổi cách QA, Review hay dashboard tính diff (ngoài phạm vi). Ghi vào báo cáo cuối: QA vẫn thấy file merge về trong diff.

### D. `src/roles/dev.rs`: kết thúc đúng nhánh
1. Sau run và **trước** kiểm tra vi phạm file: đọc nhánh hiện tại (`gitops` có sẵn hàm đọc nhánh hiện tại thì dùng, không thì thêm).
   - Khác nhánh task (biến nhánh task đã có trong hàm) thì: in `Dev kết thúc ở nhánh <x>, không phải nhánh task <y> — vi phạm giao thức`, đặt outcome `protocol_violation`, trả `EXIT_PROTOCOL`.
   - Không tự checkout lại.
2. `DEV_GRANTS_NOTE`: thêm vào cuối (trước dấu `"` kết thúc) đúng câu: ` Nếu plan khai push/pull/merge/checkout/switch: commit thay đổi trước khi đổi nhánh, merge và pull luôn kèm --no-ff (pull thêm --no-rebase), không push lên main/master, và quay về đúng nhánh task trước khi kết thúc.` Cập nhật test so khớp nguyên văn hằng này nếu có.

### E. `assets/prompts/lead.md`
Thay dòng bắt đầu `- Mục \`## Lệnh được chạy\` chỉ được khai các lệnh trên` (dòng 22) bằng **đúng** dòng sau. Backtick là backtick thật, không có `\`.
```
- Mục `## Lệnh được chạy` chỉ được khai các lệnh trên, git add/commit, script của skill, và khi việc cần thì: `git push origin *` (không bao giờ lên main/master), `git pull --no-rebase --no-ff *`, `git merge --no-ff *`, `git checkout *`, `git switch *` (chỉ để đổi hoặc tạo nhánh; Dev phải quay về nhánh task trước khi xong). Không bao giờ khai reset, rebase, restore, clean, stash, revert, cherry-pick, push --force hay xoá nhánh/tag. Khai lệnh khác thì plan bị từ chối.
```
- Dòng git-rule giữ nguyên.
- Sinh lại `brain/prompts/lead.md`.

## Test
- **`tests/guard_git.rs`**: thêm bảng; root là repo tạm có nhánh `main`, `feature/x` và file `app.php`.
  - Được phép:
    - `git push origin feature/x`, `git push -u origin feature/x`, `git push origin HEAD:feature/x`, `git push origin feature/x:feature/x`, `git push --dry-run origin feature/x`.
    - Trên nhánh `feature/x`: `git pull --no-rebase --no-ff origin feature/x`, `git merge --no-ff origin/main`, `git merge --no-ff -m "merge main" main`, `git merge --squash main`.
    - `git merge --abort`, `git checkout main`, `git checkout -b feature/y`, `git checkout -b feature/y main`, `git switch main`, `git switch -c feature/z`.
  - Bị từ chối:
    - `git push`, `git push origin`, `git push origin main`, `git push origin HEAD:main`, `git push origin x:refs/heads/master`, `git push origin MAIN`, `git push origin HEAD`, `git push origin :feature/x`, `git push origin +feature/x`, `git push -f origin feature/x`, `git push --force-with-lease origin feature/x`, `git push --delete origin feature/x`, `git push --tags origin`, `git push --mirror origin`, `git push https://evil.example/r feature/x`.
    - `git pull`, `git pull --rebase origin x`, `git pull --no-rebase origin x` (thiếu --no-ff), `git pull --no-rebase --no-ff origin main:main`.
    - `git merge main` (thiếu --no-ff), `git merge -e --no-ff main`, `git merge --no-ff -s ours main`, `git merge --continue`.
    - Khi đang ở `main`: `git merge --no-ff feature/x`, `git pull --no-rebase --no-ff origin feature/x`.
    - `git checkout -- app.php`, `git checkout app.php`, `git checkout .`, `git checkout -f main`, `git checkout -B main`, `git checkout --orphan x`, `git checkout main -- app.php`, `git checkout khong-ton-tai`, `git checkout -b main`.
    - `git switch -C x`, `git switch --discard-changes main`, `git switch -c master`.
    - `git reset --hard`, `git reset HEAD~1`, `git rebase main`, `git restore app.php`, `git clean -fd`, `git stash`, `git revert HEAD`, `git cherry-pick x`, `git branch -D x`, `git tag -d v1`.
- **Unit test `planfile`:** các mẫu hợp lệ và không hợp lệ ở mục B.
- **Unit test `gitops::dev_changed_names`** (repo tạm thật):
  1. Chỉ có commit thường: bằng `diff_names`.
  2. Nhánh task có commit sửa `a.txt`, rồi `merge --no-ff` một nhánh khác sửa `b.txt`, không xung đột: kết quả chỉ có `a.txt`.
  3. Merge có xung đột trên `c.txt`, giải tay rồi commit: kết quả có `c.txt`.
- **`tests/cli_dev.rs`** (theo kiểu test grants hiện có, dùng `fake_claude`):
  1. Fake Dev checkout sang nhánh khác rồi kết thúc ở đó: outcome `protocol_violation`, stdout có `Dev kết thúc ở nhánh`.
  2. Fake Dev commit file trong plan rồi `merge --no-ff` một nhánh có file ngoài plan: KHÔNG vi phạm.
  - Nếu fake không chạy được lệnh git tuỳ ý, mô phỏng bằng cách để test tự tạo commit merge trong repo trước khi kiểm tra; ghi cách làm vào báo cáo cuối.
- **`tests/assets_parity.rs` phải xanh.** Mọi test cũ phải xanh.

## Không được đụng tới
- `tests/assets_parity.rs`, các prompt khác ngoài `lead.md` (hai bản).
- `LEAD_ALLOWED_TOOLS`, `QA_ALLOWED_TOOLS`, `REVIEW_ALLOWED_BASE`, `GIT_READ_ONLY_TOOLS`.
- `.planning/`, `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`, các repo dự án.
- Không kill hay restart `agent-brain serve`, không dùng pkill/killall.
- Không dùng `git add -A`, `--no-verify`, không push.
- Không checkout, restore hay stash file mà bạn không sửa.
- Nếu phải sửa file ngoài bảng Thay đổi (kể cả test cũ bị đỏ do hành vi mới), được sửa **test** cũ cho đúng hành vi mới, nhưng phải liệt kê từng test trong báo cáo cuối. Không sửa `src/` ngoài bảng; nếu cần thì dừng lại và báo.

## Kiểm tra
Trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test guard_git --test cli_dev --test assets_parity 2>&1 | grep -E "^test result|FAILED"
```
Ở gốc repo:
```bash
diff <(grep -v '^- Chạy mỗi lệnh git riêng, trên một dòng' agent-brain/assets/prompts/lead.md) brain/prompts/lead.md && echo parity-ok
bash spikes/lib/scan-secrets.sh
```
Mong đợi: **mọi** test pass (không còn dòng FAILED), in ra `parity-ok`, scan-secrets OK.

## Tiêu chí nghiệm thu
1. Dev chạy được push (trừ main/master), pull, merge, checkout/switch khi plan khai; mọi dạng trong danh sách "Bị từ chối" bị chặn.
2. Lead khai được các mẫu ở mục B mà plan không bị từ chối.
3. Merge `--no-ff` không làm Dev bị đánh "sửa file ngoài plan"; file sửa khi giải xung đột vẫn bị tính.
4. Dev kết thúc sai nhánh thì bị dừng với thông báo rõ.
5. Reset, rebase, restore, clean, stash, xoá nhánh/tag, push force vẫn bị cấm.

## Commit
Hai commit:
1. `feat(agent-brain): guarded push/pull/merge/checkout/switch for Dev grants` (A, B, E và test).
2. `feat(agent-brain): Dev scope check ignores merged-in files and requires ending on the task branch` (C, D và test).

Mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối
- File đã sửa.
- Hash các commit.
- Kết quả nguyên văn các lệnh kiểm tra.
- Danh sách test cũ đã sửa, kèm lý do.
- Chỗ lệch plan, nếu có.
