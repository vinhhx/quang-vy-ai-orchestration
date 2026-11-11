# Plan: chi tiết task chia 35/65, câu trả lời xem dạng Markdown

Repo: /mnt/data/IDE/QuangVy-Orchestration (nhánh main), crate `agent-brain/`. Chủ yếu frontend. Làm trong worktree của bạn.

## Yêu cầu của người dùng
Màn chi tiết task (`#/task/<id>`): chia lại bố cục **35% – 65%**, phần **"Câu trả lời" sang bên phải (65%)** và
**xem trước (preview) được nội dung `.md`** vì nội dung raw khó đọc. **Log realtime vẫn 100% bề ngang, nằm dưới cùng.**

## Hiện trạng
- `agent-brain/assets/dashboard/screens/task.html` (874 dòng): `<section data-screen="task" x-data="task" class="space-y-6">`,
  các thẻ xếp dọc một cột: Pipeline, Sổ lượt chạy, **Answer card** (dòng ~532-550, `<pre x-text="answerMarkdown()">` là chỗ hiện raw),
  Tài liệu đầu vào, Kế hoạch, Báo cáo QA, rồi **Log card** (`#ab-log-card`, dòng ~662, `#ab-log`).
- `js/task.js` (3165 dòng): `showAnswerCard()`, `hasAnswer()`, `answerMarkdown()`, `answerPath()`, `copyAnswerPath`,
  `openHandoffDialog`. Alpine bản **CSP** (`vendor/alpine.csp.min.js`): biểu thức trong HTML chỉ được là tên thuộc tính/hàm, không viết biểu thức tự do.
- Chưa có thư viện markdown nào trong `assets/dashboard/vendor/`. Máy chạy local, KHÔNG nạp script từ CDN.
- Mọi asset được nhúng bằng `include_bytes!` trong `agent-brain/src/dashboard/assets.rs`.

## Thiết kế
1. **Bố cục hai cột** (chỉ khi có Answer card, tức `showAnswerCard()` đúng): bọc các thẻ giữa phần đầu trang và Log card trong
   `<div class="grid gap-6 xl:grid-cols-[35fr_65fr]">`:
   - cột trái (35%): mọi thẻ hiện có trừ Câu trả lời (Pipeline, Sổ lượt chạy, Tài liệu đầu vào, Kế hoạch, Báo cáo QA…),
   - cột phải (65%): thẻ Câu trả lời, `xl:sticky xl:top-4` với phần thân `xl:max-h-[calc(100vh-2rem)] overflow-y-auto`.
   Dùng breakpoint `xl` (>=1280px) vì 35% của màn hẹp hơn quá chật cho thẻ Pipeline; dưới `xl` thì xếp một cột như cũ,
   **Câu trả lời đứng trước** các thẻ còn lại.
   Task không có câu trả lời (task code): giữ nguyên một cột, không để cột phải trống.
2. **Log card giữ nguyên toàn bề ngang, ngoài lưới, cuối trang**; không đổi `#ab-log-card`, `#ab-log`, `onScroll`, chiều cao.
3. **Preview Markdown cho Câu trả lời**:
   - Thêm `agent-brain/assets/dashboard/js/markdown.js`: bộ render markdown nhỏ, **tự viết, an toàn**: nhận chuỗi, trả về
     DOM node (DocumentFragment) dựng bằng `createElement`/`textContent`, KHÔNG gán `innerHTML` từ dữ liệu, không `eval`.
     Hỗ trợ: tiêu đề `#`–`####`, đoạn văn, **đậm**, *nghiêng*, `code` dòng, khối code ``` (giữ nguyên, font mono, cuộn ngang),
     danh sách `-`/`*`/`1.` (một cấp lồng nhau), bảng pipe `| a | b |`, trích dẫn `>`, đường kẻ `---`, liên kết `[t](url)`
     (chỉ `http:`/`https:`/`#`/đường dẫn tương đối; mở bằng `rel="noopener noreferrer" target="_blank"`; `javascript:`/`data:` bị bỏ thành text).
     Ảnh `![..](..)` hiển thị dạng text liên kết, không tải ảnh. HTML thô trong markdown hiện dưới dạng text, không thực thi.
   - Hàm xuất ra `window.abMarkdown.render(text)` (theo cách các file js khác trong thư mục đang làm; xem `app.js`/`theme.js`),
     đăng ký route + `<script>` đúng như cách `js/task.js` được nạp (xem `index.html` và `src/dashboard/assets.rs`:
     thêm một mục asset `/js/markdown.js`, kiểu `application/javascript`, giống các mục js khác).
   - Trong `task.js` thêm trạng thái `answerView: 'preview'` (mặc định) và `'raw'`, hàm `setAnswerView`, `isAnswerPreview()`,
     `isAnswerRaw()`, và hàm gắn DOM vào một phần tử `x-ref` (gọi lại khi `answer.markdown` hoặc chế độ đổi; dùng `$nextTick`/`x-effect`
     theo cách Alpine CSP cho phép, hoặc `x-init` + `$watch` trong hàm khởi tạo). `copyAnswerPath`, `openHandoffDialog`, `answerPath()` không đổi.
   - Trong Answer card: hai nút chuyển **"Xem trước" / "Raw"** (nhóm nút có `aria-pressed`, vùng bấm `min-h-9`+), giữ nút "Sao chép"
     và "Gửi kết quả sang dự án khác". Raw = `<pre>` hiện tại. Preview = vùng `prose`-style viết tay bằng class Tailwind
     (không có plugin typography): khoảng cách đoạn, cỡ chữ tiêu đề, bảng có viền và cuộn ngang, code nền slate, dark mode đúng.
   - Câu trả lời dài: vùng nội dung cuộn trong thẻ (không kéo dài trang vô hạn).
4. Dựng lại CSS: `agent-brain/tools/tailwindcss` (chỉ có ở checkout chính `/mnt/data/IDE/QuangVy-Orchestration/agent-brain/tools/`; xem
   `dashboard-src/input.css` hoặc cách commit `40a5e76` đã dựng `app.css`). Chạy bằng đường dẫn tuyệt đối tới công cụ đó, đầu ra vào
   `assets/dashboard/app.css` trong worktree.

## Phạm vi file (chỉ các file này)
- `agent-brain/assets/dashboard/screens/task.html` (bọc lưới + Answer card)
- `agent-brain/assets/dashboard/js/task.js` (chỉ thêm state/hàm cho preview; không đổi hàm gọi API, id, tên hàm đang dùng)
- `agent-brain/assets/dashboard/js/markdown.js` (mới)
- `agent-brain/assets/dashboard/index.html` (chỉ thêm thẻ `<script>` nếu các js khác được nạp ở đây)
- `agent-brain/assets/dashboard/app.css` (build lại)
- `agent-brain/src/dashboard/assets.rs` (chỉ thêm một mục asset cho `/js/markdown.js`)
- test: `agent-brain/tests/dashboard_assets.rs` (thêm assert asset mới có phục vụ), và test nào bám markup cũ của Answer card
  (grep `ab-answer`, `answerMarkdown`, `Câu trả lời` trong `tests/`), sửa cho khớp, không làm yếu assertion.
Không đụng: `src/protocol_paths.rs`, `src/projects.rs`, `src/knowledge.rs` (một executor khác đang sửa), `.planning/STATE.md`,
`ROADMAP.md`, file `.planning/` chưa tracked, `~/.claude-orca`.

## Các bước
1. Đọc `task.html` (đặc biệt 525-560 và 660-690), `task.js` quanh `showAnswerCard`/`answerMarkdown`, `index.html`, `assets.rs`,
   cách các js khác được nạp, và `tests/dashboard_assets.rs`, `tests/dashboard_pipeline.rs`.
2. Viết `markdown.js` + một trang thử hoặc test node nhỏ (nếu có `node`) kiểm tra: tiêu đề, bảng, code fence, link an toàn,
   `javascript:` bị chặn, `<script>` trong markdown hiện thành text, đầu vào rỗng.
3. Đăng ký asset, nạp script, thêm state/hàm vào `task.js`, sửa Answer card, bọc lưới hai cột, giữ Log card ngoài lưới.
4. Dựng lại `app.css`, chạy test, rồi kiểm tra bằng trình duyệt headless (cách executor trước đã làm: chạy `agent-brain serve` thử với dữ liệu
   sandbox trên cổng khác, KHÔNG đụng serve đang chạy của người dùng ở cổng 7437, dừng sau khi xong) ở bề rộng 1440px và 480px,
   với một task analyst có câu trả lời markdown (tiêu đề, bảng, code, danh sách) và một task code không có câu trả lời.
5. Commit từng nhiệm vụ: `feat(dashboard): task detail split 35/65 with markdown answer preview`. Không `git add -A`, không `--no-verify`.

## Tiêu chí nghiệm thu
- >=1280px, task có câu trả lời: hai cột ~35% / 65%, Câu trả lời ở phải; các thẻ còn lại ở trái; Log realtime 100% bề ngang dưới cùng.
- <1280px: một cột, Câu trả lời trước các thẻ khác, Log vẫn cuối trang.
- Task không có câu trả lời: bố cục một cột như trước.
- Mặc định hiện Markdown đã render (tiêu đề, bảng, danh sách, code); nút "Raw" quay về văn bản gốc; "Sao chép" và "Gửi kết quả sang dự án khác" hoạt động như trước.
- Markdown chứa `<script>`, `<img onerror>`, `[x](javascript:...)` không thực thi gì; không có lỗi console.
- Log realtime vẫn cuộn/tự cuộn như trước (`#ab-log`, `onScroll` không đổi). Dark mode đúng. Cuộn ngang bảng ở màn hẹp, không vỡ trang.

## Lệnh kiểm tra (trong `agent-brain/`, từ worktree)
- `cargo test --test dashboard_assets --test dashboard_pipeline --test dashboard_projects --test dashboard_stream --test dashboard_http`
- `cargo build --release` không lỗi
- `bash spikes/lib/scan-secrets.sh` ở root repo

## Báo cáo cuối
File đã đổi, mã commit, nhánh/worktree, kết quả test, kết quả kiểm tra trình duyệt, điểm lệch plan.
Không merge, không push, không dừng/khởi động lại `agent-brain serve` đang chạy, không thay `~/.local/bin/agent-brain`.
