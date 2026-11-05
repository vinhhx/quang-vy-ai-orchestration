# Plan: Mở rộng quyền git chỉ đọc, cho `git fetch` có kiểm soát, Lead tự sửa plan sai một lần

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (đã kiểm tra)
Task #6 (dự án cnc-tech: "fetch all mới nhất về, review tiến độ ghép code, báo cáo Telegram"). Lead chạy hai lần (run 34, 35), cả hai đều có `outcome = protocol_violation` và quy trình dừng.
- `src/guard.rs` dòng 491: `GIT_SUBCOMMANDS = ["diff","log","show","status","add","commit"]`. `git_policy_check` (dòng 546) từ chối mọi subcommand khác. Ví dụ `git branch -a -vv` và `git remote -v` đều bị chặn, dù chỉ đọc.
- Plan của Lead khai `## Lệnh được chạy` gồm `git fetch --all --prune`, `git for-each-ref *`, `git branch -a --merged *`, `git rev-list *`, `git merge-base *`, `git merge-tree *`, `git ls-tree *`, `git remote -v`…
  - `planfile::grant_command_error` (dòng 709) kiểm tra lệnh git bằng `guard::bash_command_check`, nên các dòng trên đều lỗi.
  - `src/roles/lead.rs` dòng 609–622: `validate_plan` có lỗi thì run bị đánh `protocol_violation` và hàm trả `Err`. Quy trình dừng hẳn, `.plan.md` không được commit.
- Lead còn bị guard chặn khi chạy `cat .plan.md; ls -la` (toán tử `;`), vì prompt không nói rõ phải dùng Read/Glob thay cho cat/ls.
- Quyền của từng vai đi qua hai lớp, phải sửa cả hai:
  1. Guard (hook PreToolUse, chung cho mọi vai): `bash_command_check` → `git_policy_check`.
  2. Danh sách `--allowedTools` của claude theo từng vai:
     - `LEAD_ALLOWED_TOOLS` (`src/roles/lead.rs` dòng 4).
     - `QA_ALLOWED_TOOLS` (`src/roles/qa.rs` dòng 4).
     - `REVIEW_ALLOWED_BASE` (`src/roles/review.rs` dòng 8).
     - Dev: `dev_allowed_tools` cộng thêm `Bash(<lệnh>)` cho từng lệnh khai trong plan (`src/roles/dev.rs` dòng 421–424).

## Mục tiêu
1. Mọi vai được dùng thêm một nhóm lệnh git chỉ đọc. Guard cho phép các lệnh này, và allowedTools của Lead, QA, Review liệt kê chúng.
2. Lệnh `git fetch` được phép, nhưng chỉ cập nhật nhánh theo dõi từ remote đã cấu hình; không ghi đè nhánh local, không chạy chương trình ngoài.
   - Lead: có sẵn trong allowedTools.
   - Dev: dùng được khi plan khai.
   - QA, Review: KHÔNG có.
3. Plan của Lead không hợp lệ thì Lead được chạy lại thêm **một** lần, kèm danh sách lỗi để tự sửa. Lần thứ hai vẫn sai mới dừng như cũ.
4. Sửa rule trong prompt Lead cho khớp với quyền mới.
5. Không mở thêm lệnh ghi: `push`, `pull`, `merge`, `rebase`, `reset`, `checkout`, `switch`, `restore`, `clean`, `stash`, `cherry-pick`, `revert`, `tag` (tạo/xoá), `branch` (tạo/xoá/đổi tên), `remote add/remove/set-url`, `config`, `submodule`, `worktree`, `gc` vẫn bị chặn.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/guard.rs` | Mục A |
| 2 | `agent-brain/src/roles/lead.rs` | Mục B (allowedTools), Mục D (chạy lại khi plan sai) |
| 3 | `agent-brain/src/roles/qa.rs` | Mục B |
| 4 | `agent-brain/src/roles/review.rs` | Mục B |
| 5 | `agent-brain/src/roles/dev.rs` | Mục C, chỉ sửa hằng `DEV_GRANTS_NOTE` |
| 6 | `agent-brain/src/dashboard/taskstate.rs` | Chỉ thêm nhãn cho outcome `plan_invalid` (Mục D) |
| 7 | `agent-brain/assets/prompts/lead.md` | Mục E |
| 8 | `brain/prompts/lead.md` | Sinh lại từ file #7 bằng script: bỏ dòng bắt đầu `- Chạy mỗi lệnh git riêng, trên một dòng` (đúng như `without_git_rule` trong `tests/assets_parity.rs`). Không gõ tay. |
| 9 | Test: `agent-brain/tests/guard_git.rs`, `agent-brain/tests/cli_lead.rs`, test unit trong `guard.rs`/`planfile.rs`. Nếu `tests/contract_guard.rs` hay `tests/engine_guard.rs` so khớp nguyên văn câu lỗi cũ "(diff, log, show, status, add, commit)", cập nhật cho khớp. | Mục Test |

### A. `src/guard.rs`: mở rộng chính sách git

**A1. Danh sách subcommand.** Thay mảng `GIT_SUBCOMMANDS` 6 phần tử bằng hai nhóm (đổi kiểu thành `&[&str]`):
- Ghi (giữ như cũ): `add`, `commit`.
- Chỉ đọc (mới):
  - Đã có: `diff`, `log`, `show`, `status`.
  - Thêm: `branch`, `tag`, `remote`, `for-each-ref`, `show-ref`, `rev-list`, `rev-parse`, `merge-base`, `merge-tree`, `ls-tree`, `ls-files`, `cat-file`, `blame`, `describe`, `shortlog`, `name-rev`, `reflog`, `grep`, `whatchanged`, `range-diff`, `cherry`.
- Mạng: `fetch` (luật riêng ở A4).

Câu lỗi khi subcommand không có trong danh sách: `git <sub> không được phép — chỉ dùng lệnh git chỉ đọc (log, show, diff, status, branch, rev-list, merge-base…), git fetch, hoặc add/commit khi vai được ghi`. Sửa doc-comment ở dòng 490 cho đúng nghĩa mới.

**A2. Luật tùy chọn cho từng subcommand chỉ đọc.** Tận dụng vòng quét tùy chọn đang có (tiền tố long option, `GIT_LONG_EXACT_ALLOWED`); không viết parser thứ hai.
- Nhóm như `diff`, dùng `GIT_DIFF_FORBIDDEN` (`output`, `ext-diff`, `textconv`, `no-index`): `diff`, `log`, `show`, `whatchanged`, `range-diff`, `shortlog`, `rev-list`, `blame`, `grep`, `cat-file`.
- Cấm thêm:
  - `grep`: `open-files-in-pager` và cụm ngắn chứa `O`; cụm ngắn `-f` / long `file` (đọc pattern từ file).
  - `blame`: `contents`; cụm ngắn `-S` (đọc file revs).
  - `cat-file`: `filters` (chạy filter).
  - `rev-list`, `shortlog`, `log`: `stdin` (treo chờ stdin).
- `branch`: chỉ chế độ liệt kê.
  - Long option chỉ được dùng: `all`, `remotes`, `verbose`, `list`, `merged`, `no-merged`, `contains`, `no-contains`, `points-at`, `sort`, `format`, `color`, `no-color`, `column`, `no-column`, `show-current`, `ignore-case`, `abbrev`, `no-abbrev`, `quiet`. Các long option khác (`delete`, `move`, `copy`, `force`, `set-upstream-to`, `unset-upstream`, `track`, `edit-description`, `create-reflog`…) bị từ chối. Ở đây là **danh sách cho phép**, khác với kiểu danh sách cấm của các subcommand khác.
  - Cụm ngắn chỉ gồm các ký tự `a r v l i q`; mọi ký tự khác (ví dụ `d D m M c C f u t`) bị từ chối.
  - Có từ không phải tùy chọn (positional) thì bắt buộc phải có một trong `--list`/`-l`/`-a`/`--all`/`-r`/`--remotes`/`--merged`/`--no-merged`/`--contains`/`--no-contains`/`--points-at`. Nếu không, lệnh bị từ chối, vì `git branch foo` sẽ tạo nhánh.
  - Lưu ý: giá trị đi sau `--sort`, `--format`, `--merged`, `--contains`… là positional hợp lệ trong chế độ liệt kê.
- `tag`: chỉ chế độ liệt kê.
  - Long option chỉ được dùng: `list`, `sort`, `format`, `contains`, `no-contains`, `merged`, `no-merged`, `points-at`, `column`, `no-column`, `color`, `ignore-case`.
  - Cụm ngắn chỉ gồm `l n i` (`-n<số>` được phép).
  - Có positional thì bắt buộc có `-l`/`--list` hoặc một trong `contains`/`no-contains`/`merged`/`no-merged`/`points-at`.
- `remote`: chỉ được các dạng `git remote`, `git remote -v`, `git remote --verbose`, `git remote get-url <tên>`. Mọi dạng khác bị từ chối (`add`, `remove`, `rename`, `set-url`, `prune`, `update`, `show`…).
- `reflog`: chỉ `git reflog` hoặc `git reflog show …`. Từ chối `expire`, `delete`, `exists` (hay mọi từ thứ nhất khác `show` mà không bắt đầu bằng `-`).
- Các subcommand chỉ đọc còn lại: không có luật cấm riêng ngoài các luật chung.

**A3. Luật chung giữ nguyên:**
- Chỉ một lệnh trên một dòng; toán tử shell bị từ chối.
- Glob ngoài nháy bị từ chối.
- Tùy chọn toàn cục chỉ được `-P` và `--no-pager`.
- Không gọi qua lệnh bọc, không dùng `git-<x>`.
- `ONE_COMMAND_RULE` không đổi.

**A4. `git fetch`.**
- Long option chỉ được dùng: `all`, `prune`, `prune-tags`, `tags`, `no-tags`, `quiet`, `verbose`, `dry-run`.
  - Mọi long option khác bị từ chối, nhất là `force`, `upload-pack`, `update-head-ok`, `refmap`, `recurse-submodules`, `set-upstream`, `write-fetch-head`.
- Cụm ngắn chỉ gồm `p P q v t`. Từ chối `u` (update-head-ok), `f`, `k`, `j`, `m`.
- Positional (tên remote hoặc tên nhánh):
  - Chỉ gồm ký tự `[A-Za-z0-9._/-]`.
  - Không bắt đầu bằng `-`, `/` hay `.`.
  - Không chứa `..`, `:` hay `+`.
  - Như vậy chặn được refspec ghi đè nhánh local (`main:main`, `+refs/...`), URL (`https://…`, `ext::…`) và đường dẫn file.
- Câu lỗi ví dụ: `git fetch chỉ được cập nhật nhánh theo dõi từ remote đã cấu hình (ví dụ: git fetch --all --prune, git fetch origin) — <chi tiết>`.

**A5. `planfile::grant_command_error`** đang gọi `bash_command_check`, nên tự nhận luật mới; không sửa `planfile.rs`, trừ khi test cho thấy phải sửa. Kết quả mong đợi: toàn bộ 14 dòng `Lệnh được chạy` của task #6 dưới đây đều hợp lệ (dòng `notify.sh` hiện đã hợp lệ, phải giữ hợp lệ):
```
git fetch --all --prune
git for-each-ref *
git branch -a --merged *
git branch -a --no-merged *
git rev-list *
git merge-base *
git merge-tree *
git log *
git show *
git diff *
git ls-tree *
git status *
git remote -v
.claude/skills/fix-bugs/scripts/notify.sh *
```

### B. allowedTools theo vai
Thêm vào `LEAD_ALLOWED_TOOLS`, `QA_ALLOWED_TOOLS`, `REVIEW_ALLOWED_BASE` các mục `Bash(git <sub> *)` cho **mọi subcommand chỉ đọc** ở A1, cùng `Bash(git branch)`, `Bash(git tag)`, `Bash(git remote)`, `Bash(git reflog)` (dạng không đối số, vì `git x *` của claude có thể không khớp lệnh không đối số; kiểm tra test hiện có xem `git status *` đang khớp `git status` thế nào và làm theo đúng cách đó).
- Chỉ Lead có thêm `Bash(git fetch *)` và `Bash(git fetch)`.
- Đổi kích thước mảng hằng cho khớp, hoặc chuyển sang `&[&str]` nếu gọn hơn (giữ tên hằng).
- Để tránh lặp ba lần: thêm trong `guard.rs` một `pub const GIT_READ_ONLY_TOOLS: &[&str]` (các chuỗi `Bash(git … *)`), rồi ba vai nối vào danh sách của mình. Tên hằng chọn tuỳ ý nhưng phải dùng chung.
- Dev không đổi `dev_allowed_tools`: Dev chỉ có thêm lệnh git khi plan khai (dòng 421–424 đã làm việc này).

### C. `src/roles/dev.rs`: `DEV_GRANTS_NOTE`
Thêm vào cuối hằng (trước dấu `"` kết thúc) đúng câu: ` Nếu plan khai git fetch: chạy git fetch đúng như đã khai, không thêm refspec có dấu hai chấm và không fetch từ URL.`
- Tìm mọi test so khớp nguyên văn hằng này và cập nhật cho khớp.

### D. `src/roles/lead.rs`: plan sai thì Lead tự sửa một lần

Hiện tại (dòng 609–622): `validate_plan` có lỗi thì đánh `protocol_violation` và trả `Err`.

Hành vi mới:
1. **Lần 1 có lỗi validate:**
   - Đặt outcome của run lần 1 là `plan_invalid`, KHÔNG phải `protocol_violation`.
   - In ra (`term::emit_out`) dòng `Plan không hợp lệ — Lead tự sửa lại (lần 2/2):` rồi danh sách lỗi.
   - Chạy Lead **lần 2** ngay trong cùng `cmd_lead`.
2. **Đầu vào của lần 2:**
   - Dùng chuỗi mở đầu của nhánh `keep_plan` (sửa trực tiếp trên `.plan.md` hiện tại, giữ hai dòng đầu, mỗi dòng bảng Thay đổi một file), với cùng `Yêu cầu`/`Task`.
   - Nối thêm đúng đoạn:
     ```
     \nLần chạy trước viết .plan.md không hợp lệ. Lỗi:\n<mỗi lỗi một dòng "  - <lỗi>">\nChỉ sửa .plan.md để hết các lỗi trên: bỏ hoặc thay lệnh không được phép bằng lệnh hợp lệ, giữ nguyên phần còn lại của plan.
     ```
   - Giữ nguyên khối git context (khi Lead là agy) và khối skill như lần 1.
   - Giữ ghi chú người dùng (`note`) nếu có.
3. **Lần 2:**
   - Dùng `run_dir` mới (`new_run_dir`), `settings` mới, cùng engine, model, timeout và **cùng giá trị `round`** với lần 1.
   - Chạy các kiểm tra sau run như lần 1: timeout/killed/failed, agy rỗng, file ngoài `.plan.md`, thiếu `.plan.md`, meta.
4. **Lần 2 vẫn lỗi validate:**
   - Làm như hành vi cũ: outcome `protocol_violation`, trả `Err` với thông báo cũ.
   - Thêm vào đầu thông báo câu `Lead đã tự sửa 1 lần nhưng plan vẫn không hợp lệ.`
5. **Không chạy lại** trong các trường hợp sau, giữ nguyên hành vi cũ:
   - Lead sửa file ngoài `.plan.md`.
   - Không có `.plan.md`.
   - Run timeout, killed hoặc failed.
   - `--dry-run`.
6. **Cách làm gọn:** tách phần "dựng RoleSpec → argv → `execute_role_staged` → kiểm tra sau run → validate" thành một hàm hoặc closure, gọi tối đa 2 lần. Không nhân đôi khối code dài.
   - Phần commit `.plan.md`, `clear_task_approval`, `set_task_status` chỉ chạy một lần, sau lần hợp lệ.
   - Lưu ý quy tắc `plan_unchanged`/agy rỗng: ở lần 2, `initial_plan_text` là nội dung `.plan.md` ngay trước lần 2.
7. **Đếm vòng:**
   - Kiểm tra `autopilot::decide::effective_rounds` và `tests/lead_rounds.rs`: hai run lead cùng `round` không được làm tăng số vòng hay kích hoạt giới hạn vòng.
   - Run lần 1 có `outcome = plan_invalid` và status thành công. Autopilot không được coi run này là lần plan xong để chạy bước tiếp theo, vì cùng tiến trình đã có run lần 2 sau nó.
   - Nếu `effective_rounds`/`decide` cần biết `plan_invalid` thì sửa tối thiểu và ghi lại trong báo cáo cuối. Nếu phải sửa file ngoài bảng Thay đổi, dừng lại và báo.
8. **`src/dashboard/taskstate.rs`** khoảng dòng 105: thêm `Some("plan_invalid") => return ("sua_lai", "Plan sai, Lead sửa lại"),`.

### E. `assets/prompts/lead.md`: sửa rule
1. Giữ nguyên dòng git-rule (`- Chạy mỗi lệnh git riêng, trên một dòng…`). Dòng này chỉ được có đúng **một** dòng và không được đổi nội dung (`tests/assets_parity.rs` kiểm tra).
2. Ngay **trước** dòng git-rule, thêm đúng hai dòng sau:
```
- Khi khảo sát: đọc file bằng Read, tìm file bằng Glob/Grep; không dùng cat, ls, head hay ghép nhiều lệnh. Lệnh git được phép: chỉ đọc (log, show, diff, status, branch, tag, remote -v, for-each-ref, rev-list, rev-parse, merge-base, merge-tree, ls-tree, ls-files, cat-file, blame, describe, shortlog, reflog, grep) và git fetch (ví dụ `git fetch --all --prune`, `git fetch origin`; không refspec có dấu `:`, không URL).
- Mục `## Lệnh được chạy` chỉ được khai các lệnh trên, cộng git add/commit và script của skill. Không khai push, pull, merge, rebase, reset, checkout, switch, stash, clean, tạo hay xoá nhánh/tag. Khai lệnh khác thì plan bị từ chối.
```
3. Trong dòng `+ Nội dung ở commit hay nhánh khác…`, đổi `ví dụ các dòng: - \`git show *\`, - \`git diff *\`, - \`git log *\`` thành `ví dụ các dòng: - \`git show *\`, - \`git diff *\`, - \`git log *\`, - \`git branch -a --no-merged *\`, - \`git fetch --all --prune\``. Các backtick trong file là backtick thật, không có dấu `\`. Dấu `\` ở đây chỉ để hiển thị markdown của plan này.
4. Sinh `brain/prompts/lead.md` theo dòng #8 của bảng.

KHÔNG sửa `dev.md`, `qa.md`, `code-review.md`, `plan-review.md`, `lead-untrusted.md` hay bản trong `brain/prompts/` của chúng.

## Test
- **`tests/guard_git.rs`** (theo dạng bảng allowed/refused đang có trong test, hoặc `check_table` trong `guard.rs`):
  - Được phép:
    - `git branch`, `git branch -a`, `git branch -a -vv`, `git branch -r --merged origin/main`, `git branch -a --no-merged 'origin/main'`, `git branch --show-current`.
    - `git remote`, `git remote -v`, `git remote get-url origin`.
    - `git for-each-ref --sort=-committerdate refs/remotes`, `git rev-list --count main..origin/main`, `git merge-base main origin/dev`, `git merge-tree --write-tree main origin/dev`.
    - `git ls-tree -r HEAD`, `git ls-files`, `git blame app/a.php`, `git describe --tags`, `git shortlog -sn`, `git cat-file -p HEAD`, `git tag`, `git tag -l 'v1*'`, `git reflog`, `git grep foo`, `git rev-parse HEAD`.
    - `git fetch`, `git fetch --all --prune`, `git fetch origin`, `git fetch origin feature/x`, `git fetch -p origin`.
  - Bị từ chối:
    - `git branch foo`, `git branch -d foo`, `git branch -D foo`, `git branch -m a b`, `git branch --delete foo`, `git branch -f main HEAD`, `git branch --set-upstream-to=origin/x`.
    - `git tag v1`, `git tag -d v1`.
    - `git remote add x https://e`, `git remote set-url origin x`, `git remote remove origin`.
    - `git reflog expire --all`, `git reflog delete HEAD@{1}` (lệnh này còn bị chặn bởi `{`; vẫn liệt kê).
    - `git fetch origin main:main`, `git fetch origin +refs/heads/x`, `git fetch https://evil.example/x`, `git fetch ext::sh`, `git fetch ../other`, `git fetch --upload-pack=x origin`, `git fetch -u origin`, `git fetch --force origin`.
    - `git grep -O foo`, `git grep --open-files-in-pager foo`, `git blame --contents x a.php`, `git cat-file --filters HEAD:a`.
    - `git push`, `git pull`, `git reset --hard`, `git checkout main`, `git switch x`, `git stash`, `git clean -fd`, `git merge x`, `git rebase x`, `git config user.name x`.
- **Unit test `planfile`:** plan có 14 dòng `Lệnh được chạy` của task #6 (A5) cho `validate_grants` hoặc `validate_plan` không có lỗi về lệnh. Plan khai `git push *` hoặc `git fetch origin main:main` thì có lỗi.
- **allowedTools:** test (unit hoặc `cli_*` dry-run) xác nhận:
  - Lead có `Bash(git branch *)` và `Bash(git fetch *)`.
  - QA và Review có `Bash(git branch *)` nhưng **không** có `Bash(git fetch *)`.
- **`tests/cli_lead.rs`** (dùng `fake_claude` như các test hiện có):
  1. Lần 1 ghi plan có `## Lệnh được chạy` chứa `git push *`, lần 2 sửa thành `git log *`. Kết quả:
     - Lệnh thoát 0.
     - Có 2 run lead: run 1 có outcome `plan_invalid`, run 2 thành công.
     - Prompt lần 2 chứa `Lần chạy trước viết .plan.md không hợp lệ`.
     - `.plan.md` được commit với message `<slug>: plan`.
  2. Cả hai lần đều ghi `git push *`. Kết quả:
     - Lệnh lỗi.
     - Run 2 có outcome `protocol_violation`.
     - Thông báo có `Lead đã tự sửa 1 lần`.
     - Không có lần chạy thứ 3.
  3. Lần 1 sửa file ngoài `.plan.md`: vẫn `protocol_violation` ngay, không chạy lần 2 (hành vi cũ).
  - Nếu `fake_claude` chưa hỗ trợ hành vi khác nhau theo từng lần gọi, thêm cách đơn giản (ví dụ biến môi trường trỏ tới file đếm số lần gọi) trong chính file helper của test. Ghi vào báo cáo cuối file đã sửa thêm.
- **`tests/assets_parity.rs` phải XANH** (không sửa file này).

## Không được đụng tới
- `tests/assets_parity.rs`, các prompt khác ngoài `lead.md` (cả hai bản).
- `.planning/` (kể cả file plan này), `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`.
- Repo cnc-tech, everon và mọi repo dự án khác.
- Không kill hay restart `agent-brain serve` (cổng 7437), không dùng pkill/killall.
- Không dùng `git add -A`, không dùng `--no-verify`, không push.
- Không checkout, restore hay stash file mà bạn không sửa.

## Kiểm tra
Trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "^(warning|error)" | sort | uniq -c
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test assets_parity --test guard_git --test cli_lead 2>&1 | grep -E "^test result|FAILED"
```
Ở gốc repo:
```bash
diff <(grep -v '^- Chạy mỗi lệnh git riêng, trên một dòng' agent-brain/assets/prompts/lead.md) brain/prompts/lead.md && echo parity-ok
bash spikes/lib/scan-secrets.sh
```
Mong đợi:
- **Mọi** test pass.
- Clippy không có warning mới so với trước khi sửa (chạy clippy một lần trước khi sửa để có số đối chiếu).
- In ra `parity-ok`.
- scan-secrets OK.

## Tiêu chí nghiệm thu
1. Guard cho phép các lệnh git chỉ đọc ở A1 và `git fetch` có kiểm soát; mọi lệnh ở danh sách "Bị từ chối" vẫn bị chặn.
2. 14 dòng `Lệnh được chạy` của task #6 hợp lệ.
3. Lead, QA, Review có allowedTools git chỉ đọc mới; chỉ Lead có `git fetch`.
4. Plan sai thì Lead tự sửa thêm đúng một lần; sai lần 2 mới dừng với `protocol_violation`.
5. Prompt Lead có rule mới; parity xanh.

## Commit
Hai commit:
1. `feat(agent-brain): read-only git commands and guarded git fetch for agents` (A, B, C, E và test liên quan).
2. `feat(agent-brain): Lead retries once when its plan fails validation` (D và test liên quan).

Mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối
- File đã sửa.
- Hash các commit.
- Kết quả nguyên văn các lệnh kiểm tra.
- Chỗ lệch plan, nếu có. Đặc biệt nêu nếu phải đụng `autopilot/` hay helper `fake_claude`.
