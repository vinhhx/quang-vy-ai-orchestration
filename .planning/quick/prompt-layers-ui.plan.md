# Plan B: sửa ghi chú prompt trên dashboard, gợi ý stack khi quét dự án, mẫu dựng sẵn cho các stack đang dùng

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Làm SAU Plan A (`.planning/quick/prompt-layers-engine.plan.md`, đã merge: có `src/prompts.rs`, `Project.stack`, `agent-brain prompt show`). Đọc toàn bộ file này và Plan A trước khi làm.

## Mục tiêu
1. API ghi chú (`dashboard/api_prompts.rs` mới, gắn vào router có xác thực token/cookie như các route settings):
   - `GET /api/prompts` → danh sách `{ layers: { stacks: [...tên có file hoặc mẫu...], projects: [...], workflows: [...], teams: [...] }, roles: [...], builtin_stacks: [...] }`.
   - `GET /api/prompts/{layer}/{name}/{role}` → `{ text, source: "user"|"builtin"|"none", version, max_bytes }`.
   - `PUT /api/prompts/{layer}/{name}/{role}` body `{ text, version }` → ghi `<config_dir>/prompts/<layer>/<name>/<role>.md` (tạo thư mục), kiểm: tên/vai hợp lệ (`prompts::valid_name`, vai trong danh sách), ≤ 8192 byte, version khớp (xung đột → 409 `code:"conflict"`). Ghi atomically (file tạm + rename, như `settings_store`).
   - `DELETE /api/prompts/{layer}/{name}/{role}` → xoá file người dùng (về mẫu dựng sẵn nếu có). Với stack có mẫu dựng sẵn, nút "Tắt mẫu" = PUT text rỗng.
   - `GET /api/prompts/preview?project=<id>&role=<vai>&workflow=<tên?>&team=<tên?>` → `{ prompt, warnings }` dùng đúng `prompts::compose` + `prompts::layers_for` (không chạy gì).
2. Stack của dự án:
   - `PUT /api/projects/{id}/stack` body `{ stack: [..] }` ghi trường `stack` trong `projects_map.json` (dùng cơ chế ghi map có sẵn, kiểm tên), tối đa 6 stack.
   - Gợi ý khi quét/thêm dự án (`projects.rs::scan`, `add_project`): hàm `pub fn detect_stack(root: &Path) -> Vec<String>` (chỉ đọc tên file ở gốc, không đọc nội dung lớn): `artisan` → `laravel`; `yii` hoặc `protected/yiic` hoặc `config/web.php`+`vendor/yiisoft` → `yii2`; `composer.json` (không phải 2 cái trên) → `php`; `package.json` → `node`; `.agents/skills/srt-idea` hoặc `scripts/build_video.py` → `video-srt`; `Cargo.toml` → `rust`; `pyproject.toml`/`requirements.txt` → `python`. Chỉ ĐIỀN cho dự án chưa có trường `stack` (không ghi đè lựa chọn của người dùng). Thêm vào kết quả `scan` số dự án được gợi ý stack.
3. Giao diện (Alpine CSP; KHÔNG truy cập theo khoá động `obj[expr]` trong thuộc tính; boolean attribute chỉ bind vào method trả boolean thật — xem commit 1b72b99; mọi nút có `focus-visible:outline-2 …`; sáng/tối):
   - Màn **Dự án**: mỗi dự án có ô chọn stack (chip, thêm/bỏ, gợi ý từ `builtin_stacks` + stack đã dùng) và nút "Ghi chú prompt" mở panel: tab theo vai (lead/dev/qa/plan-review/code-review), textarea ghi chú dự án, đếm byte/8192, Lưu (xử lý 409), và nút "Xem prompt cuối cùng" (chọn workflow/team, gọi preview, hiện trong khung chỉ đọc cuộn được).
   - Màn **Cài đặt**: thêm mục "Ghi chú prompt" gồm 3 nhóm: Stack (danh sách stack + mẫu dựng sẵn, sửa theo vai, nhãn "mẫu dựng sẵn"/"đã sửa", nút Khôi phục mẫu), Workflow (theo tên workflow hiện có), Đội hình (theo tên team hiện có); cùng editor như trên.
4. Mẫu dựng sẵn (nhúng binary) `agent-brain/assets/prompts/stacks/<stack>/<vai>.md` + bảng `include_str!` trong `prompts.rs` (`builtin_stack_note`). Viết ngắn gọn (mỗi file ≤ 1500 byte), tiếng Việt, chỉ là luật nghiệp vụ, KHÔNG nói về quyền:
   - `video-srt/lead.md`: chạy script bằng `.venv/bin/python scripts/<tên>.py` (khai đúng dạng này trong `## Lệnh được chạy`); KHÔNG đưa `scripts/**` vào thư mục ghi, không lập plan sửa script dùng chung; code riêng của video đặt trong `projects/<tên>/`; người dùng đã duyệt plan = đã duyệt Gate 1–4 → `## Tạo sản phẩm` gồm `create_project.py <tên> --minutes …`, `build_video.py --stage script`, `build_video.py --stage all [--voice …]`; `## Sản phẩm` = `projects/<tên>/final/<tên>-complete.mp4`; đọc `AGENTS.md`, `VIDEO_WORKFLOW.md`.
   - `video-srt/dev.md`: chạy đúng các lệnh `## Tạo sản phẩm` theo thứ tự, ở foreground, chờ xong; lỗi giọng đọc thì dừng và ghi Vướng mắc, không đổi giọng; không sửa `scripts/`.
   - `video-srt/qa.md`: kiểm `ffprobe` file sản phẩm (độ phân giải 1920x1080, có audio, thời lượng trong khoảng `video.json`); nghe/đối chiếu phụ đề không bắt buộc.
   - `laravel/lead.md`, `laravel/dev.md`, `laravel/qa.md`: lệnh kiểm `php artisan test` (hoặc `vendor/bin/phpunit`), không sửa `.env`/`config/*` có secret, migration mới thay vì sửa migration cũ, tuân PSR-12; QA chạy test liên quan.
   - `yii2/lead.md`, `yii2/dev.md`, `yii2/qa.md`: tương tự cho Yii2 (`vendor/bin/codecept run` nếu có `codeception.yml`, migration bằng `yii migrate/create`, cấu hình trong `config/` không chứa secret mới).
   Đọc nhanh 1–2 dự án thật cùng stack (CHỈ ĐỌC, ví dụ một thư mục dưới `/mnt/data/source_codes/code/php/company/laravel/` và `/yii2/`) để mẫu khớp thực tế (lệnh test có tồn tại không); ghi trong tóm tắt đã đọc dự án nào.

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/dashboard/api_prompts.rs` (mới), `agent-brain/src/dashboard/mod.rs` (gắn router) | Mục tiêu 1 |
| 2 | `agent-brain/src/dashboard/api_projects.rs`, `agent-brain/src/projects.rs` | Mục tiêu 2 |
| 3 | `agent-brain/src/prompts.rs` | bảng mẫu dựng sẵn (Mục tiêu 4), hàm liệt kê cho API |
| 4 | `agent-brain/assets/prompts/stacks/**` (mới) | Mục tiêu 4 |
| 5 | `agent-brain/assets/dashboard/screens/projects.html`, `js/projects.js`, `screens/settings.html`, `js/settings.js`, `app.css` (sinh lại) | Mục tiêu 3 |
| 6 | `scripts/dashboard-js-check.mjs` | kiểm tra cho UI mới |
| 7 | Test: `agent-brain/tests/dashboard_prompts.rs` (mới), `tests/dashboard_projects.rs`, `tests/contract_scan.rs` (gợi ý stack), unit trong `projects.rs`/`prompts.rs` | Test |

Không sửa: `assets/prompts/*.md` gốc của vai, `~/.config/agent-brain/*` thật, `~/.local/share/agent-brain/*`, các dự án thật (chỉ đọc), `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác.

## Test (bắt buộc)
- (a) API: GET/PUT/DELETE đúng file trong config tạm; 8193 byte → 400; tên `..`/vai lạ → 400; version cũ → 409; preview khớp `agent-brain prompt show` cho cùng tham số.
- (b) Stack: PUT ghi đúng map; `detect_stack` với thư mục giả cho từng loại; `scan` chỉ điền dự án chưa có `stack`.
- (c) Mẫu dựng sẵn được nhúng (mỗi file ≤ 1500 byte, không rỗng), `builtin_stack_note("video-srt","lead")` có `.venv/bin/python`.
- (d) Toàn bộ test cũ pass; `dashboard-js-check` pass (gồm luật không `obj[expr]`).

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration
bash scripts/build-css.sh && bash scripts/build-css.sh --check
bash scripts/dashboard-ui-lint.sh
node scripts/dashboard-js-check.mjs
cd agent-brain && cargo test --no-fail-fast
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"   # không tăng so với base
git diff --stat <HEAD trước khi chạy>..HEAD
```
Trình duyệt (bắt buộc): chạy serve THỬ ở cổng khác (ví dụ 7601) với `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG` trỏ vào thư mục tạm và KHÔNG có `telegram.json` (để không tranh bot với serve thật); KHÔNG dừng hay đụng serve thật ở cổng 7437; chỉ dừng PID mình khởi chạy (không `pkill`/`killall`). Kiểm: sửa stack của một dự án, sửa ghi chú vai lead, xem prompt cuối có khối Stack/Dự án đúng thứ tự; khôi phục mẫu; lỗi 409 hiện đúng. Chụp ảnh (sáng/tối) và ghi đường dẫn tuyệt đối.

## Commit
Rust (Mục tiêu 1, 2, 4) một commit `feat(agent-brain): prompt notes API, project stacks and built-in stack notes`; UI một commit `feat(dashboard): edit prompt notes and project stacks`. `git add` từng file, không `git add -A`, không `--no-verify`, message kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: file sửa, commit, chỗ lệch plan, kết quả kiểm tra, ảnh chụp, dự án thật đã đọc.
