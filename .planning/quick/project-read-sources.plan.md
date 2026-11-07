# Plan: dự án nguồn chỉ đọc (task của dự án A đọc được code dự án B)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Chạy **trước** `task-handoff.plan.md` (chạy sau `departments-rules.plan.md`). Đọc toàn bộ file này trước khi làm.

## Bối cảnh (quyết định người dùng 2026-11-07)
Dự án `automation-test-e2e` viết e2e cho các website của team, ví dụ `apec`. Agent của task e2e cần **chủ động đọc code apec** (route, form, validate, quyền) để viết test case rồi test. Hiện tại:
- `planfile::grant_dir_error` từ chối mọi đường dẫn ngoài dự án;
- guard (`guard.rs`) chỉ xét đường dẫn dưới `policy.root`;
- claude chỉ đọc trong cwd.

Plan này thêm liên kết **ở mức dự án**. Người dùng khai báo trên dashboard: "dự án `automation-test-e2e` được đọc dự án `apec`". Mọi task của `automation-test-e2e` khi đó đọc được toàn bộ repo apec, **không bao giờ ghi**, và không đọc file nhạy cảm (`.env*`, `.git`, danh sách `report::sensitive_path`).

Liên kết do người dùng tự đặt là sự cho phép rõ ràng, nên plan không cần cú pháp cấp quyền riêng.

## Thay đổi

### 1. Khai báo trong `projects_map.json` (`src/projects.rs`, `src/dashboard/api_projects.rs`)
- Mục dự án có thêm trường tùy chọn `read_sources: ["apec", …]` (id dự án đã đăng ký). `Project` thêm `read_sources: Vec<String>`. `from_value` đọc trường này, thiếu thì là mảng rỗng.
- Hàm `projects::read_source_paths(map, project) -> (Vec<(String id, PathBuf realpath)>, Vec<String> warnings)`. Bỏ qua kèm cảnh báo các trường hợp:
  - id không tồn tại;
  - id trùng chính dự án;
  - dự án `missing`;
  - thư mục không tồn tại;
  - realpath của nguồn nằm trong, hoặc chứa, thư mục của dự án hiện tại (tránh chồng lấn với quyền ghi).
- `POST /api/projects/{id}/read-sources` `{read_sources: [...]}` theo mẫu `update_stack` (ghi nguyên tử, chống ghi đè). Validate:
  - mỗi id có thật và khác chính nó;
  - tối đa 5 nguồn;
  - không trùng lặp.

  Lỗi trả theo trường. `GET /api/projects` trả thêm `read_sources`.
- Màn hình **Dự án** (`screens/projects.html`, `js/projects.js`): trong phần sửa dự án thêm mục "Dự án nguồn (chỉ đọc)". Gồm danh sách chọn nhiều dự án khác, kèm dòng giải thích "Task của dự án này được đọc code các dự án nguồn, không ghi, không đọc .env". Theo cách sửa `stack` hiện có.

### 2. Prompt: cho agent biết có nguồn (`src/prompts.rs` hoặc seam `roles/mod.rs`)
- Ở seam `execute_role_staged` (ngay sau chỗ nối `departments::prompt_append_for_member` / `append_to_system_prompt`): nếu dự án của task có nguồn hợp lệ, nối block:
  ```
  ## Dự án nguồn (chỉ đọc)
  - apec: /mnt/.../apec — thư mục tri thức: .gsd (nếu có)
  Đọc bằng đường dẫn tuyệt đối. Không ghi, không chạy lệnh trong các thư mục này, không đọc file .env*.
  ```
  Thư mục tri thức lấy bằng `crate::knowledge` (hàm hiện có tìm `.gsd` / `.planning` / `.agent-brain`).
- Cảnh báo của `read_source_paths` đi vào `warnings` của run.

### 3. Claude: cho đọc thư mục nguồn (`src/roles/mod.rs` `build_command_*`, hoặc chỗ mỗi vai dựng `RoleSpec`)
Với mọi vai chạy claude của task có nguồn (lead, dev, qa, plan-review, code-review, analyst, analysis-review), với mỗi nguồn `P`:
- thêm `--add-dir <P>` vào argv. Kiểm `check_forbidden_args` vẫn qua;
- thêm vào allowed: `Read(/<P>/**)` (dùng `abs_rules`, ra dạng `Read(//mnt/...)`);
- thêm vào disallowed:
  - `Read(/<P>/**/.env*)`, `Read(/<P>/.env*)`, `Read(/<P>/.git/**)`;
  - `Edit(/<P>/**)`, `Write(/<P>/**)`.

Không thêm quyền Bash nào cho nguồn.

### 4. Guard: chỉ đọc trong nguồn (`src/guard.rs`)
- `Policy` thêm `source_roots: Vec<PathBuf>` (tuyệt đối, đã realpath). File policy cũ thiếu trường thì coi là rỗng. Theo cách `read_files` được thêm, gồm `path_list("source_roots")` và ghi trong `write_hook_settings*`.
- `GuardGrants` (và các hàm `write_hook_settings` / `write_hook_settings_grants`) nhận `source_roots`. `roles/dev.rs` và `roles/review.rs` truyền vào từ `read_source_paths`.
- `decide`:
  - **Ghi** (Write/Edit/NotebookEdit…) dưới một `source_root` luôn bị từ chối, kể cả khi có trong write list. Lý do: `"thuộc dự án nguồn chỉ đọc"`.
  - **Đọc**, khi `read_allow` là `Some`: cho phép nếu `resolved` nằm dưới một `source_root` và `grant_denied_rel(rel_to_that_root)` là false. Lý do: `"in read-only source project"`.
  - Grep/Glob đang bị cấm khi `read_allow` được đặt: giữ nguyên với đường dẫn trong dự án. Cho phép khi đường dẫn tìm kiếm nằm dưới một `source_root` và không bị `grant_denied_rel`.
- `agy_listing_decision` và `agy_tool_decision_inner`: cùng luật, để agy (giám sát qua stream ở `roles/mod.rs`) bị chặn giống claude.
- `decide_bash`: không đổi. Lệnh bash không được nới vì có nguồn.
- Symlink: so sánh trên realpath (`realpath_nfc`) như các luật hiện có. Symlink trong nguồn trỏ ra ngoài nguồn thì không được đọc.

### 5. agy
- agy dựng prompt từ argv claude (`claude_argv_parts`), nên block mục 2 tự có.
- Kiểm tra agy CLI có cần cờ thêm thư mục làm việc để đọc ngoài cwd không. Chạy `agy --help` (chỉ xem help, không chạy agent) và tìm cờ kiểu `--add-dir` / `--workspace`. Có thì thêm cho mỗi nguồn trong `agy_plan`. Không có thì ghi vào tóm tắt, coi guard mục 4 là lớp chặn.

### 6. Dry-run và xem trước
- `agent-brain prompt show` và `GET /api/prompts/preview` hiện block nguồn.
- `agent-brain projects` (lệnh liệt kê hoặc xem dự án hiện có) hiện `read_sources`.

### 7. Test
- `projects`:
  - đọc `read_sources`;
  - `read_source_paths` bỏ id lạ, chính nó, thư mục thiếu, nguồn chồng lấn, kèm cảnh báo.
- API: lưu hợp lệ; id lạ, chính nó, quá 5 → lỗi trường; xung đột version.
- Argv claude cho một task có nguồn:
  - có `--add-dir <P>`, `Read(//<P>/**)`;
  - có các luật disallow `.env*`, `.git`, `Edit` / `Write`.
- Guard (dùng thư mục tạm có `src/` và `.env` giả bằng chuỗi vô hại):
  - đọc `P/src/a.php` → cho phép;
  - đọc `P/.env` và `P/sub/.env.local` → từ chối;
  - ghi `P/src/a.php` → từ chối, kể cả khi đường dẫn có trong write list;
  - symlink trong `P` trỏ ra ngoài → từ chối;
  - Grep dưới `P` → cho phép, Grep dưới root dự án khi `read_allow` đặt → vẫn từ chối;
  - agy `read_file` / `list_dir` / `write_file` cùng kết quả.
- Policy file cũ không có `source_roots` vẫn đọc được.
- Prompt: block "Dự án nguồn (chỉ đọc)" có trong argv claude và agy (fake) của task có nguồn. Task không có nguồn thì không có block.
- Không chạy claude hay agy thật. Dùng `HOME` / `AGENT_BRAIN_*` tạm.

## Không sửa
`Cargo.toml`, `~/.config/agent-brain/*` và `~/.local/share/agent-brain/*` thật, dashboard thật cổng 7437, repo ngoài, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác, `~/.claude-orca`. Không nới `grant_dir_error`: grant trong plan vẫn chỉ trong dự án. Không cho ghi hay chạy lệnh trong nguồn.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # không tăng so với 153
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
bash spikes/lib/scan-secrets.sh
```

## Commit
Hai commit:
1. `feat(agent-brain): read-only source projects for a project's tasks`: mục 1 (phần projects.rs và API), 2, 3, 4, 5, 6 và test.
2. `feat(dashboard): edit a project's read-only source projects`: phần giao diện của mục 1.

`git add` đúng file, không `-A`, không `--no-verify`, không push. Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Tóm tắt gồm: file sửa, commit, chỗ lệch plan, kết quả mục 5 (agy có cờ thêm thư mục không), clippy trước/sau, kết quả kiểm tra.
