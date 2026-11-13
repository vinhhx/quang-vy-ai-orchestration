# Plan worktree-php-env: worktree của task PHP chạy được test (vendor + container tạm) và sửa kiểm "có code sau plan" (2026-11-14)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. **Đọc toàn bộ file này trước khi làm.** Mỗi mục số (1..3) là một commit riêng, `git add` đúng các file đã sửa (không `git add -A`, không `--no-verify`).

## Bối cảnh (đã điều tra, task 66 messer và 67 cnc-tech)
Từ commit `1f27cae` (11/11) task mới chạy trong worktree riêng `~/.local/share/agent-brain/worktrees/<dự án>/<slug>`. Với dự án Laravel có ba lỗi:
1. **Worktree không có `vendor/`** (thư mục bị gitignore). `php artisan test` chết ngay: `vendor/autoload.php: No such file`. Hiện chỉ `node_modules` được nối (`src/workspace.rs:99 link_node_modules`).
   - **Không được symlink `vendor`**: trong `vendor/modules/*` là symlink tương đối `../../app-modules/<x>/` (composer path repo), và autoload dùng `__DIR__ . '/../..'`. Qua symlink, PHP resolve về repo chính ⇒ test chạy code của repo chính, không phải code của task. Phải **copy thật**. Worktree (`/home`, ext4) và repo (`/mnt/data`) khác ổ ⇒ không hardlink được.
   - `git worktree remove` (không `--force`) vẫn xoá worktree có `vendor/` bị ignore (đã thử), nên `agent-brain task cleanup` giải phóng được dung lượng.
2. **PHP trên máy thiếu extension** so với container (`pdo_sqlite`, `sqlite3`, `gd`, `bcmath`), test dùng sqlite `:memory:` ⇒ `could not find driver`. Container dự án (`messer_app`, `cnc-tech_app`) chỉ mount repo chính, nên `decide()` (`src/container.rs:590`) bỏ shim và cảnh báo "Container … không mount worktree của task — lệnh php, composer chạy trên máy".
   - Đã chạy tay thành công: `docker run --rm -u 1000:1000 --network none -v <wt>:<wt> -v <ini>:<ini dest>:ro -w <wt> -e APP_KEY=… messer-app php artisan test …`.
3. **Kiểm "chưa có thay đổi code sau plan" sai khi Lead lập lại plan**: task 66 — Dev round 1 commit code rồi báo `dev:blocked`; Lead viết lại plan (commit `…: plan` mới); Dev round 2 thấy đã làm hết nên không commit ⇒ `diff_code_names(<plan commit MỚI NHẤT>, HEAD)` rỗng ⇒ autopilot dừng `run_loi` ở code-review dù code đã có. Neo ở plan commit mới nhất: `src/roles/review.rs:178-184`, `src/roles/qa.rs:240-252`, `src/autopilot/mod.rs:503-509`, `src/dashboard/api_tasks.rs:1747-1764`.

## Kiến thức nền (đường dẫn trong `agent-brain/`)
- `src/workspace.rs`: `link_node_modules` (:99), `ensure_node_modules_excluded`, `add_worktree` (:136), `create_task_worktree` (:166), `ensure_task_root` (:182; nhánh tái dùng worktree :195-200). Thêm một call site ở `src/roles/lead.rs:583-590`.
- `src/projects.rs`: option `link_node_modules` (`LINK_NODE_MODULES_KEY` :43, field :93, default :104, parse :139) — làm y hệt cho option mới.
- `src/container.rs`: `INSPECT_FORMAT` (:32, **chỉ** lấy tên/trạng thái/mounts — không lấy env vì chứa secret), `ContainerInfo` (:37), `Inspected`/`inspected_of` (:136-181), `mount_for` (:219), `shim_safe` (:475), `shim_script` (:484), `write_shims` (:524), `prompt_block` (:561), `decide` (:590), `for_run` (:625), test giả `FakeProbe` (~:705).
- `src/gitops.rs`: `all_plan_commits_on_branch` (mới nhất trước), `plan_commit_on_branch` (:376, lấy phần tử đầu), `diff_code_names` (:736, đã loại file protocol).

## Phạm vi file (chỉ sửa trong)
- `agent-brain/src/workspace.rs`, `agent-brain/src/projects.rs`, `agent-brain/src/roles/lead.rs` (chỉ call site :583-590 nếu chữ ký đổi).
- `agent-brain/src/container.rs`, và nơi kết thúc/kill run để dọn container tạm (xem 2.5; ghi rõ file trong tóm tắt).
- `agent-brain/src/gitops.rs`, `agent-brain/src/roles/review.rs`, `agent-brain/src/roles/qa.rs`, `agent-brain/src/autopilot/mod.rs`, `agent-brain/src/dashboard/api_tasks.rs`.
- Test `#[cfg(test)]` trong các file trên; test tích hợp mới `agent-brain/tests/worktree_php_env.rs` nếu cần.
- `agent-brain/README.md`: chỉ **thêm** một đoạn ngắn trong phần worktree (append-only).

## Không đụng
`.planning/STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md`, file `.planning/` chưa track (ví dụ `.planning/milestone.lock`), `~/.config/agent-brain/**`, `~/.local/share/agent-brain/**`, binary `~/.local/bin/agent-brain`, `~/.claude-orca`. **Không** cài binary, **không** restart `agent-brain serve`, **không** chạy `docker` thật trong test (dùng probe/giả lập). Không đổi hành vi dự án ở chế độ shared hoặc khi container đã mount được worktree.

## 0. Trước khi sửa
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
git rev-parse HEAD
cargo test --offline --locked --no-fail-fast -- --test-threads=1 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
```
Ghi HEAD và số nền vào tóm tắt. Test đỏ sẵn: ghi tên, không sửa.

## 1. Copy `vendor` vào worktree (commit `workspace: copy vendor vào worktree của dự án PHP`)
1.1 `src/projects.rs`: option mới `copy_vendor` (key `"copy_vendor"`, bool, mặc định **true**), đọc giống hệt `link_node_modules` (giá trị không phải bool ⇒ giữ mặc định). Test parse như test của `link_node_modules` (~:1878, :1910).

1.2 `src/workspace.rs`: `pub fn copy_vendor(project_root: &Path, worktree: &Path)` — mọi lỗi chỉ là cảnh báo (`crate::term::emit`), không bao giờ làm hỏng việc tạo worktree:
- Bỏ qua nếu `project_root/composer.json` không phải file, hoặc `project_root/vendor` không phải thư mục thật (symlink_metadata).
- `worktree/vendor` đã là **thư mục thật** ⇒ không làm gì (tái dùng được, gọi lại an toàn).
- `worktree/vendor` là **symlink** ⇒ xoá symlink (chỉ symlink, `remove_file`) rồi copy, kèm cảnh báo giải thích (symlink làm autoload trỏ về repo chính).
- Copy: xoá thư mục tạm cũ `worktree/.vendor.agent-brain-tmp` nếu có, chạy `cp -a --reflink=auto <root>/vendor <worktree>/.vendor.agent-brain-tmp` (`std::process::Command`, không qua shell), thành công thì `rename` sang `worktree/vendor`; thất bại thì xoá thư mục tạm và cảnh báo (kèm stderr cắt ngắn). Symlink tương đối bên trong (`vendor/modules/x -> ../../app-modules/x/`) phải giữ nguyên dạng tương đối (`cp -a` làm đúng).
- In một dòng `Đã copy vendor vào worktree (N MB)` (đếm nhanh bằng `du -sm` hoặc tự cộng kích thước; không bắt buộc chính xác — nếu khó thì bỏ số MB).
- Thêm `vendor/` và `.vendor.agent-brain-tmp/` vào `info/exclude` của worktree giống `ensure_node_modules_excluded` (tổng quát hoá hàm đó thành nhận danh sách mẫu nếu gọn hơn), để worktree vẫn "sạch" với kiểm dirty của Lead.
- Gọi `copy_vendor` ở mọi chỗ đang gọi `link_node_modules`: `add_worktree` (cả nhánh tái dùng và nhánh tạo mới) và `ensure_task_root` (nhánh worktree đã có, :195-200) — theo option `copy_vendor` của dự án. Nhánh tái dùng là để task đang chạy dở (ví dụ task 67) tự có `vendor` ở lần chạy sau. Đổi chữ ký `add_worktree`/`create_task_worktree` cho gọn (ví dụ truyền `&WorkspaceSettings` hoặc thêm bool) và sửa call site `src/roles/lead.rs:583-590`.

1.3 Test (`#[cfg(test)]` trong `workspace.rs`, tempdir): repo giả có `composer.json`, `vendor/autoload.php`, `vendor/modules/x -> ../../app-modules/x/` và `app-modules/x/`; worktree giả có `app-modules/x/`:
- sau `copy_vendor`: `worktree/vendor/autoload.php` tồn tại; `canonicalize(worktree/vendor/modules/x)` nằm **trong worktree**; không còn thư mục tạm.
- gọi lần 2 không lỗi, không copy lại (đổi nội dung file trong repo rồi gọi lại ⇒ worktree giữ bản cũ).
- worktree có `vendor` là symlink ⇒ sau gọi là thư mục thật.
- không có `composer.json` ⇒ không tạo gì.
- `git status --porcelain` trong worktree (git thật trong tempdir) không liệt kê `vendor`.

## 2. Chạy lệnh dự án trong container tạm khi container không mount worktree (commit `container: chạy lệnh của task worktree trong container tạm cùng image`)
2.1 Dò: mở rộng `INSPECT_FORMAT` thêm **chỉ** `"Config":{"Image":{{json .Config.Image}}}` (tuyệt đối không thêm `.Config.Env`). `Inspected` thêm `image: String`. `ContainerInfo` thêm `image: String` và `extra_binds: Vec<(PathBuf, String)>` = mọi bind mount của container **trừ** mount repo đã chọn (`mount_for`), giữ nguyên source/destination (ví dụ `messer/setup/docker/php/custom.php.ini -> /usr/local/etc/php/conf.d/custom.ini`). Sửa `FakeProbe`/test hiện có cho trường mới.

2.2 `decide()` (:590): khi `cwd` **không** nằm trong `info.mount_source` **và** `cwd` là worktree của task (nằm dưới thư mục gốc worktrees của agent-brain — dùng hàm có sẵn trong `workspace.rs` cho đường dẫn gốc, đừng hard-code) **và** `info.image` không rỗng ⇒ bật chế độ **container tạm** thay vì cảnh báo "chạy trên máy". Biểu diễn: thêm enum chế độ vào `RunContainer.active` (ví dụ `Exec` hiện tại / `Ephemeral { worktree: PathBuf }`), giữ nguyên hành vi `Exec` cũ. Cảnh báo mới (đưa vào `warnings` để hiện ở metrics/prompt): `Container {name} không mount worktree — lệnh {cmds} chạy trong container tạm từ image {image}`. Không đủ điều kiện ⇒ giữ cảnh báo cũ.

2.3 Shim chế độ tạm (hàm mới cạnh `shim_script`, mọi giá trị qua `shim_safe`; ảnh/tên container kiểm hợp lệ; từ chối ⇒ quay về cảnh báo "chạy trên máy" như hiện nay):
```sh
#!/bin/sh
# agent-brain: chạy '<cmd>' trong container tạm từ image <image> (mạng của <name>)
case "$PWD/" in
  "<worktree>/"*) ;;
  *) echo "agent-brain: $PWD không nằm trong worktree <worktree>" >&2; exit 97 ;;
esac
exec "<docker>" run --rm -i \
  --label agent-brain.run=<run_id> \
  -u "$(id -u):$(id -g)" -e HOME=/tmp \
  --network "container:<name>" \
  -v "<worktree>:<worktree>" \
  [-v "<project_root>/.env:<worktree>/.env:ro"]      # chỉ khi file .env của repo chính tồn tại lúc viết shim VÀ worktree không có .env riêng
  [-v "<extra src>:<extra dest>:ro" ...]             # extra_binds
  -w "$PWD" "<image>" <cmd> "$@"
```
- Mount worktree ở **cùng đường dẫn tuyệt đối** để đường dẫn trong output trùng máy.
- `--network container:<name>` dùng mạng của container dự án (tới được DB/redis theo tên service nếu test cần).
- `.env` chỉ được mount vào container (agent không đọc được qua tool trên máy); **không** copy `.env` vào worktree, không in nội dung.
- `<run_id>`: định danh run (id DB hoặc tên thư mục run), dùng ở 2.5.
- `prompt_block` cho chế độ tạm: nói lệnh chạy trong container tạm từ image X, thư mục làm việc = worktree (đường dẫn giống máy), không gọi `docker` trực tiếp.

2.4 `write_shims` / nơi gọi `for_run` dùng đúng shim theo chế độ. Guard/allowlist lệnh của agent giữ nguyên (agent vẫn gọi `php artisan test …`; shim nằm đầu `PATH`).

2.5 Dọn container mồ côi: khi run kết thúc hoặc bị kill (tìm chỗ ghi `ended_at`/xử lý kill của run — ghi tên file trong tóm tắt), nếu run dùng chế độ tạm thì chạy `docker ps -q --filter label=agent-brain.run=<run_id>` rồi `docker rm -f` các id đó (timeout 10 giây, lỗi chỉ cảnh báo). Lý do: `SIGKILL` vào client `docker run` không dừng container.

2.6 Test (không chạy docker thật): parse inspect có `Config.Image`; `decide` với cwd trong worktree + container mount repo chính ⇒ chế độ tạm, cảnh báo mới; image rỗng ⇒ cảnh báo cũ; cwd trong mount ⇒ `Exec` như cũ; nội dung shim chứa `run --rm -i`, `--network "container:<name>"`, `-v "<wt>:<wt>"`, extra bind `:ro`, có/không dòng `.env` đúng điều kiện; giá trị có ký tự `$`/`"`/`` ` `` ⇒ từ chối.

## 3. Neo "có code sau plan" ở plan commit ĐẦU TIÊN của task (commit `review/qa: tính code từ plan đầu tiên của task`)
3.1 `src/gitops.rs`: `pub fn first_plan_commit_on_branch(root, branch, slug, base_commit) -> Result<Option<String>, ()>` = phần tử **cuối** của `all_plan_commits_on_branch` (cũ nhất). Test: nhánh có `slug: plan` → commit code → `slug: plan` (lập lại) ⇒ `first` trả commit plan cũ, `plan_commit_on_branch` vẫn trả commit mới nhất.

3.2 Dùng anchor mới cho **kiểm có code** và cho **diff đưa reviewer/QA** (để reviewer thấy code làm trước lần lập lại plan):
- `src/roles/review.rs:178-186` (kiểm `ReviewKind::Code` và `protocol.code_diff_command(...)`), `src/roles/qa.rs:240-255` (kiểm và `code_diff_command`), `src/autopilot/mod.rs:503-509` (`code_after_plan`), `src/dashboard/api_tasks.rs:1757-1764` (`code_after_plan`).
- Lấy anchor qua một hàm phụ duy nhất (ví dụ `crate::roles::code_anchor_for(...)` cạnh `plan_commit_for` trong `src/roles/mod.rs` — được phép thêm vào file đó) để bốn chỗ không lệch nhau. Trường hợp không tìm được (chạy ngoài nhánh task, `find_commit_by_subject`) giữ hành vi cũ.
- **Không** đổi các chỗ khác dùng plan commit mới nhất (duyệt plan, `approved_commit`, đọc plan, `split::plan_split_items`, `dev.rs`).
3.3 Test: tái hiện task 66 — base → `plan` → code (Dev round 1) → `plan` (lập lại) → không commit thêm ⇒ `code_after_plan == true`, review Code không còn lỗi "chưa có thay đổi code sau plan", diff reviewer chứa file code. Ca không có code nào sau plan đầu ⇒ vẫn báo lỗi như cũ.

## Kiểm tra (chạy lại toàn bộ sau mục 3)
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --release --offline --locked
cargo test --offline --locked --no-fail-fast -- --test-threads=1 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cd .. && bash spikes/lib/scan-secrets.sh
```
Đạt khi: build xanh; failed = 0 (hoặc chỉ các test đỏ sẵn đã ghi ở bước 0); số passed ≥ nền + test mới; scan-secrets OK.

## Tiêu chí nghiệm thu
- Worktree mới (hoặc worktree cũ ở lần chạy kế) của dự án có `composer.json` có `vendor/` là **thư mục thật**, `vendor/modules/*` resolve vào worktree; `git status` worktree sạch.
- Run của task worktree trên dự án có container (mount repo chính) dùng shim `docker run --rm` cùng image, mạng của container, mount worktree cùng đường dẫn; container tạm bị dọn khi run kết thúc/bị kill.
- Không lộ env container, không copy `.env` vào worktree.
- Task có code commit trước lần lập lại plan không còn bị chặn "chưa có thay đổi code sau plan"; reviewer/QA thấy diff code đó.
- Hành vi shared mode và container mount được worktree không đổi.

## Tóm tắt phải ghi (`.planning/quick/worktree-php-env.summary.md`, commit riêng `docs: tóm tắt worktree-php-env`)
HEAD trước/sau, các commit, file đã sửa (kể cả file dọn container ở 2.5), số test trước/sau, lệch plan và lý do.
