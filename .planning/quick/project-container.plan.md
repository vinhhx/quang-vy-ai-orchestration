# Plan project-container: chạy lệnh dự án trong container docker đang chạy (2026-11-09)

Mục tiêu: trên máy có container docker đang chạy mount repo dự án (vd `bizsurvey_app`: `/mnt/data/source_codes/code/php/company/laravel/bizsurvey` → `/var/www`, PHP 8.3 có `pdo_sqlite`), lệnh `php` / `composer` mà agent chạy trong task được chuyển vào container đó. **Tự dò, không cần cấu hình.** Máy không có container như vậy (máy đồng nghiệp) thì chạy như cũ.

Lý do không cho plan khai `docker exec`: `docker` nằm trong `FORBIDDEN_PROGRAMS` (`src/planfile.rs:828`) và phải giữ nguyên. Agent chỉ thấy lệnh `php`/`composer`; việc gọi docker nằm trong shim do agent-brain viết.

Code hiện tại: `main` tại HEAD lúc bắt đầu (ghi lại). Chỉ sửa trong:
- `agent-brain/src/**`, `agent-brain/tests/**`, `agent-brain/assets/**`, `agent-brain/README.md`

## 0. Trước khi sửa
Ghi số test/clippy nền như các plan trước:
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"
```

## 1. Dò container (`src/container.rs`, mới)
- `pub struct ContainerInfo { name: String, source: PathBuf /* thư mục máy được mount */, dest: String /* đường dẫn trong container */ }`.
- `pub fn detect(project: &Project) -> Option<ContainerInfo>` dùng probe tiêm vào được (`detect_with(project, probe)`) để test không cần docker thật:
  1. `docker ps --format '{{.ID}}'` (timeout 5 s). Lỗi / không có docker / không quyền ⇒ `None`, chỉ `tracing::debug!`.
  2. `docker inspect <các id>` (một lệnh, JSON). Với mỗi container, xét `Mounts` loại `bind`: lấy mount có `Source` (realpath) bằng gốc repo dự án hoặc là thư mục cha của nó; chọn mount có `Source` dài nhất. `dest` = `Destination` + phần còn lại của đường dẫn (nếu Source là thư mục cha).
  3. Nhiều container khớp (vd app + queue): ưu tiên theo thứ tự
     a. tên nằm trong `container_name:` của file compose trong repo: `setup/docker/**/*compose*.y*ml`, `setup/local/**/*compose*.y*ml`, `docker/**/*compose*.y*ml`, `docker-compose*.y*ml`, `compose*.y*ml` ở gốc (đọc dòng `container_name:` bằng chuỗi, không cần thư viện YAML; tối đa 20 file, mỗi file ≤ 256 KiB). Trong số đó ưu tiên tên kết thúc `_app` hoặc `-app`;
     b. tên kết thúc `_app` / `-app`;
     c. tên chứa id dự án;
     d. tên nhỏ nhất theo thứ tự chữ.
  4. Bỏ container không ở trạng thái running.
- Cache kết quả theo đường dẫn dự án 60 giây (Mutex + HashMap, thời điểm dò), để mỗi run không gọi docker nhiều lần.
- Đường dẫn chương trình docker: tìm `docker` trong `PATH` của tiến trình agent-brain lúc dò, lưu đường dẫn tuyệt đối vào `ContainerInfo.docker_bin`.

## 2. Ghi đè từng máy (không bắt buộc)
`projects_map.json`, mỗi dự án nhận khoá tuỳ chọn `container`:
- không có hoặc `"auto"`: dò như mục 1 (mặc định);
- `false`: tắt, chạy trên máy;
- `{"name": "<tên>"}`: chỉ dùng container tên này (vẫn kiểm tra đang chạy và có mount repo; không đạt ⇒ coi như không có, ghi cảnh báo).
- `"commands"` (trong object, tuỳ chọn): danh sách lệnh chuyển vào container, mặc định `["php", "composer"]`. Chỉ nhận tên `[a-z0-9._-]{1,32}`, không nhận `docker`, `sh`, `bash` hay tên trong `FORBIDDEN_PROGRAMS`.
Đọc/ghi theo cách các khoá `workspace`, `base_branch` đang làm trong `src/projects.rs` (validate, giữ khoá lạ khi ghi lại).

## 3. Shim lệnh trong run
Ở chỗ dựng `env` cho tiến trình engine của mỗi run vai trò (lead, dev, qa, plan-review, code-review, analyst, fix… — tìm nơi dựng `RunRequest.env`/`req.env` được `src/runner.rs:418` dùng):
- Nếu dự án có container (mục 1–2) **và** thư mục làm việc của run (`cwd` của engine, tức gốc task) nằm trong `source` của mount: tạo `<run_dir>/bin/` (0700) với một file cho mỗi lệnh trong `commands` (0700):
  ```sh
  #!/bin/sh
  # agent-brain: chạy '<cmd>' trong container <name>
  case "$PWD/" in
    "<source>/"*) rel="${PWD#<source>}" ;;
    *) echo "agent-brain: $PWD không nằm trong thư mục được mount vào container <name>" >&2; exit 97 ;;
  esac
  exec <docker_bin> exec -i -w "<dest>$rel" <name> <cmd> "$@"
  ```
  Các giá trị `<…>` được kiểm tra trước khi ghi: chỉ chấp nhận đường dẫn/tên không có ký tự `'"$\``, xuống dòng hay khoảng trắng đầu/cuối; không đạt ⇒ không tạo shim, ghi cảnh báo.
- Prepend `<run_dir>/bin` vào `PATH` trong env của engine.
- Không tạo shim khi: dự án không có container, chế độ worktree mà worktree không nằm trong `source`, hoặc `container: false`. Khi không tạo vì worktree không được mount: thêm cảnh báo vào `metrics_json.warnings` của run: `"Container <name> không mount worktree của task — lệnh php chạy trên máy"`.
- Ghi vào `metrics_json` của run: `{"container": {"name": …, "dest": …, "commands": […]}}` để dashboard/kiểm tra sau.

## 4. Báo cho agent
Khi shim bật, thêm vào cuối prompt hệ thống của vai trò (cùng chỗ ghép các lớp ghi chú) một khối ngắn:
```
Môi trường: lệnh `php`, `composer` của dự án này chạy trong container docker `<name>` (thư mục repo = `<dest>`). Gọi chúng như bình thường từ thư mục repo; đường dẫn trong output là đường dẫn trong container. Không gọi `docker` trực tiếp.
```
Danh sách lệnh lấy từ `commands`.

## 5. Lệnh kiểm tra
`agent-brain container PROJECT`: in kết quả dò (tên container, source → dest, lệnh, lý do chọn nếu nhiều container khớp, hoặc lý do không có). Không in biến môi trường của container.

## 6. Test
Dùng probe giả (JSON `docker inspect` mẫu), không gọi docker thật:
- 1 container mount đúng gốc repo ⇒ chọn; mount thư mục cha ⇒ `dest` có phần còn lại; không mount ⇒ `None`; container dừng ⇒ bỏ.
- 2 container khớp, repo có `setup/docker/docker-compose.yml` với `container_name: x_app` ⇒ chọn `x_app`; không có compose ⇒ ưu tiên `_app`.
- `container: false` ⇒ `None`; `{"name": "khac"}` không mount ⇒ `None` + cảnh báo; `commands` có `docker` ⇒ lỗi validate.
- Shim: nội dung đúng (đường dẫn docker tuyệt đối, `-w` đúng), quyền 0700, `PATH` có `<run_dir>/bin` đứng đầu; ký tự cấm trong tên ⇒ không tạo.
- Chạy thật shim với một `docker` giả (script in ra tham số) trong thư mục tạm: `php -v` từ thư mục con ⇒ tham số `exec -i -w <dest>/<con> <name> php -v`; từ ngoài source ⇒ exit 97.
- Worktree không nằm trong source ⇒ không shim, có cảnh báo.

## 7. Kiểm tra thật (sau khi test xanh, không cần dashboard)
Trên máy này (có `bizsurvey_app`):
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
./target/release/agent-brain container bizsurvey     # thấy bizsurvey_app, /mnt/.../bizsurvey -> /var/www
./target/release/agent-brain container apec          # thấy apec_app
./target/release/agent-brain container automation-test-e2e   # không có container
```
Kiểm tra Bash tool của Claude Code giữ shim đứng đầu PATH: viết một test bỏ qua mặc định (`#[ignore]`) hoặc một lệnh tay ghi trong tóm tắt, chạy `claude -p "Chạy lệnh: command -v php; php -r 'echo extension_loaded(\"pdo_sqlite\") ? \"ok\" : \"no\";'" --model claude-haiku-5-5` với env `PATH=<thư mục shim>:$PATH` trong thư mục repo bizsurvey (chỉ đọc), và ghi kết quả vào tóm tắt. Nếu Bash tool của Claude Code đặt lại PATH làm shim không đứng đầu, **dừng và báo lại** (không tự đổi cách làm sang hook rewrite).

## Không sửa
- `FORBIDDEN_PROGRAMS` và luật grant trong `src/planfile.rs`.
- `Cargo.toml` (không thêm crate; JSON đã có `serde_json`).
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, `~/.local/bin/agent-brain`, dashboard đang chạy cổng 7437 (KHÔNG cài binary, KHÔNG khởi động lại serve — có task đang chạy).
- Repo dự án ngoài. Không chạy `docker` thật ngoài `docker ps` / `docker inspect` ở mục 7.
- `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` khác, `~/.claude-orca`.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --release
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'   # > nền, failed 0
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # ≤ nền
grep -n '"docker"' src/planfile.rs                                # vẫn trong FORBIDDEN_PROGRAMS
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
bash spikes/lib/scan-secrets.sh
```

## Commit
Một commit: `feat(agent-brain): run project php/composer inside the project's running docker container`. `git add` đúng file, không `-A`, không `--no-verify`, không push. Tóm tắt: file sửa, commit, số test/clippy, kết quả mục 7, chỗ lệch plan.
