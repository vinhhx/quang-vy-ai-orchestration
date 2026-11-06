# Plan: dashboard phòng ban: sửa thành viên, đổi độ khó, chấm điểm

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. **Chạy sau** `departments-core.plan.md`: dùng `departments.rs`, `tasks.level`, `proposals.level`, `runs.member_id`, `member_scores` của plan đó. Đọc toàn bộ file này trước khi làm.

## Bối cảnh (quyết định người dùng 2026-11-06)
- Người dùng tự chọn model và mức độ cho từng thành viên, và chấm điểm trên **dashboard**. Telegram chỉ dùng để xử lý việc, plan này không thêm gì vào Telegram.
- Theo đúng quy ước dashboard hiện có: Alpine + Tailwind đã build, mỗi màn hình là `assets/dashboard/screens/<x>.html` và `assets/dashboard/js/<x>.js`. Mọi nút có `aria-label`, giao diện tiếng Việt.

## Thay đổi

### 1. API (`agent-brain/src/dashboard/api_departments.rs` mới, đăng ký ở `dashboard/mod.rs`)
Theo mẫu `api_settings.rs` (teams/staff: đọc, ghi nguyên tử, chống ghi đè khi file bị sửa nơi khác).
- `GET /api/departments` trả về:
  - năm phòng, mỗi phòng có `{id, name, roles[]}`;
  - mỗi thành viên có `{id, display_name, engine, model, levels, notes, ready, ready_reason, stats}`, với `stats` gồm `runs`, `succeeded`, `avg_score` (null nếu 0 lượt), `scores`, `running`;
  - `errors` và `version` (mtime/hash để chống xung đột, như teams).
- `POST /api/departments/member/save` `{department, id, member, version}`: kiểm bằng `departments::validate_entry`, trả lỗi theo trường (`FieldError`). Đổi phòng của một thành viên được phép (xóa ở phòng cũ, thêm ở phòng mới).
- `POST /api/departments/member/delete` `{id, version}`.
- `GET /api/departments/models?engine=<id>` trả danh sách gợi ý model:
  - `claude` / `anthropic` native: hằng `["claude-opus-5-5","claude-sonnet-5-5","claude-haiku-5-5"]` cộng các model đang có trong `config.json`.
  - agy: chạy `<agy command> models` với stdin `/dev/null`, timeout 20 giây, cache 10 phút trong bộ nhớ, lấy cột đầu mỗi dòng có tab. Lỗi thì trả mảng rỗng kèm `error`.
  - Khác: trả `models` trong config.
- `POST /api/tasks/{id}/level` và `POST /api/proposals/{id}/level` `{level}`: theo mẫu `post_proposal_priority`. Task đang có run `running` thì từ chối với lời nhắn tiếng Việt.
- `POST /api/runs/{id}/score` `{score: 1..5, note?}`:
  - chỉ cho run có `member_id` và đã kết thúc;
  - upsert theo `run_id`, `note` ≤ 500 ký tự;
  - trả về điểm TB mới của thành viên.
- `GET /api/tasks/{id}` và danh sách run: thêm `level`, và mỗi run thêm `member_id`, `member_name`, `score`, `score_note`.

### 2. Màn hình "Phòng ban" (`screens/departments.html`, `js/departments.js`, mục điều hướng trong `index.html`, đăng ký trong `app.js`)
- Thêm mục điều hướng `#/departments`, `aria-label="Phòng ban"`, đặt sau "Dự án", icon nhóm người (SVG cùng kiểu các icon hiện có).
- Năm khối phòng. Mỗi khối có bảng thành viên với các cột: Tên · Engine · Model · Mức độ (chip Dễ/Vừa/Khó) · Sẵn sàng · Điểm TB (số lượt) · Thành công/Tổng run · Đang chạy. Thêm nút "Thêm thành viên" và nút Sửa/Xóa mỗi dòng.
- Form sửa:
  - mã (khóa lại khi sửa), tên, phòng (select);
  - engine (select từ danh sách engine hiện có, không có `người`);
  - model (`input` có `datalist` gợi ý từ `/api/departments/models`, vẫn cho gõ tay);
  - mức độ (3 checkbox), ghi chú.

  Lỗi theo trường hiện dưới ô.
- Phòng không có thành viên hiện dòng "Chưa có thành viên — vai của phòng chạy theo đội hình." Có `errors` thì hiện khối cảnh báo ở đầu màn hình.
- Khi chưa có file: hướng dẫn ngắn, có nút "Tạo từ mẫu". Nút gọi `POST /api/departments/init-example`, chép `assets/departments.example.json` (nhúng bằng `include_str!` riêng cho endpoint này) vào `departments.json` chỉ khi file chưa có.

### 3. Độ khó và chấm điểm trên màn hình task và hàng đợi (`screens/task.html`, `js/task.js`, `screens/queue.html`, `js/queue.js`)
- Thẻ đề xuất ở hàng đợi có select Độ khó, đặt cạnh select ưu tiên hiện có.
- Màn hình task:
  - có chip hoặc select Độ khó ở phần đầu;
  - mỗi run hiện `member_name` (nếu có);
  - run đã kết thúc có `member_id` thì có 5 nút sao (`aria-label="Chấm 3 điểm"`…) và ô ghi chú ngắn kèm nút Lưu. Đã chấm thì hiện điểm và cho sửa.
- Đội hình `phong-ban` hiện ở select đội hình như các đội hình khác (nếu select đang đọc `/api/teams` thì tự có).

### 4. Test (`agent-brain/tests/dashboard_departments.rs` mới)
Theo mẫu `dashboard_settings.rs`, gồm:
- GET khi file rỗng hoặc không có;
- save hợp lệ;
- save lỗi trường (engine `người:x`, level rỗng, mã trùng ở phòng khác);
- xung đột `version`;
- delete;
- `init-example` không ghi đè file có sẵn;
- level của task và proposal (giá trị lạ → 400, task đang chạy → từ chối);
- score: run không có `member_id` → 400, run đang chạy → 400, upsert, cập nhật `avg_score`;
- models agy bằng fake `agy` in hai dòng có tab;
- `GET /api/tasks/{id}` có `level`, `member_name`, `score`.

Thêm khóa kiểm tra vào `scripts/dashboard-js-check.mjs` và `scripts/dashboard-keys.mjs` nếu các script đó yêu cầu khai báo màn hình hoặc khóa mới.

## Không sửa
`Cargo.toml`, `~/.config/agent-brain/*` và `~/.local/share/agent-brain/*` thật, dashboard thật cổng 7437, repo ngoài, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác, `~/.claude-orca`. Không thêm lệnh hay nút Telegram.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # không tăng so với sau plan core
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check     # nếu báo lệch thì chạy build-css.sh rồi commit app.css
spikes/lib/scan-secrets.sh
```

## Commit
Hai commit:
1. `feat(dashboard): departments screen and member editor`: mục 1 (phần departments, models, init-example), mục 2 và test.
2. `feat(dashboard): task level and member scoring`: phần level, score và task/run JSON của mục 1, mục 3 và test.

`git add` đúng file, không `-A`, không `--no-verify`, không push. Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Tóm tắt gồm: file sửa, commit, chỗ lệch plan, clippy trước/sau, kết quả kiểm tra.
