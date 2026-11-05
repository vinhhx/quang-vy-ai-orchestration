# Plan: gộp Tổng quan và Giao việc vào màn Điều phối (mở nhanh bằng popup)

Repo `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Mục tiêu
Người dùng chỉ làm việc trên **một** màn chính là Điều phối (`#/queue`).
- Menu bỏ hai mục "Tổng quan" và "Giao việc".
- Từ màn Điều phối, nội dung đầy đủ của hai màn đó mở nhanh bằng **popup** (hộp thoại modal), không cần rời màn.
- Trang mặc định (`#/`) là Điều phối.

## Bối cảnh (đã kiểm tra trong code)
- **Router** `assets/dashboard/js/app.js`:
  - `parseHash` (khoảng dòng 475–505): `#/` và hash lạ trả về `overview`.
  - `loadScript(name)` nạp `/js/<name>.js` một lần; `navigate()` fetch `/screens/<name>.html`, gán vào `#screen` rồi gọi `Alpine.initTree`.
  - Khi đổi màn, router xoá mọi timer trong `window.AB._timers` (đăng ký qua `AB.every`) và gọi các hook trong `window.AB._leaveHooks` (đăng ký qua `AB.onLeave`).
  - Component đăng ký bằng `AB.onReady(name, factory)`, tức `Alpine.data`.
- **Menu:** `assets/dashboard/index.html`, dòng 17–18 (Tổng quan `#/`, Giao việc `#/assign`) và dòng 19 (Điều phối `#/queue`).
- **Các link liên quan:**
  - `screens/overview.html` dòng 6 và 22: `href="#/assign"`.
  - `screens/task.html` dòng 15 và 22: `href="#/"` với chữ "← Tổng quan".
- **Màn Giao việc** (`js/assign.js`, `screens/assign.html`): khi orchestrator bật, màn này gửi vào `/api/intake`; khi tắt, tạo task trực tiếp. Hãy xem khi gửi thành công màn làm gì (có chuyển hash sang `#/task/{id}` không).
- **Màn Tổng quan** (`js/overview.js`, `screens/overview.html`): các nhóm task, có polling qua `AB.every`.
- **Màn Điều phối** (`js/queue.js`, `screens/queue.html`): có thanh trạng thái và ô giao nhanh ở đầu màn.
- **Kiểm tra hiện có:**
  - `scripts/dashboard-smoke.sh`: mặc định chạy các màn `overview` và `assign`; dòng 740–741 đặt hash `overview → #/`, `assign → #/assign`.
  - `scripts/dashboard-js-check.mjs`: test các factory `overview`, `assign` và `board`.
  - `scripts/dashboard-ui-lint.sh`.
  - `tests/dashboard_assets.rs`: danh sách màn trong `asset_references_resolve`.

## Thiết kế

### 1. Cơ chế popup dùng chung (`app.js`)
Thêm `window.AB.screenModal = { open(name), close(), isOpen() }`, với `name` thuộc `['overview', 'assign']`.

**Khi `open(name)`:**
- Nạp script bằng `loadScript(name)`. Hàm này hiện nằm trong closure của router: đưa ra một hàm dùng chung, không viết bản thứ hai.
- Fetch `/screens/<name>.html`, chèn vào phần thân popup, rồi gọi `Alpine.initTree(phần thân)`.

**Phạm vi timer và hook:**
- Timer (`AB.every`) và hook (`AB.onLeave`) tạo ra bởi component trong popup phải thuộc **phạm vi của popup**: đóng popup thì phải xoá các timer đó và gọi các hook đó, còn timer và hook của màn Điều phối bên dưới vẫn giữ nguyên.
- Cách làm gợi ý: `AB` có ngăn xếp phạm vi. `open` đẩy một phạm vi mới; `AB.every`/`AB.onLeave` ghi vào phạm vi đang ở đỉnh; `close` dọn phạm vi đó rồi bỏ nó ra.
- Lưu ý: component có thể gọi `AB.every` sau một `await`, khi phạm vi đỉnh có thể đã khác. Trong `overview.js` và `assign.js`, xác định chỗ nào gọi `every`/`onLeave` không đồng bộ. Nếu có, giữ phạm vi đúng: ví dụ `open` gắn phạm vi vào phần tử gốc của popup và component lấy phạm vi theo phần tử, hoặc bất kỳ cách nào chứng minh được bằng test. Ghi cách đã chọn vào báo cáo.

**Khi đổi màn** (`navigate`): đóng popup đang mở trước, dọn cả phạm vi popup.

**Giao diện popup:**
- Lớp nền mờ; khung `max-w-5xl`, cao tối đa `90vh`, cuộn bên trong. Trên điện thoại chiếm toàn màn hình.
- Tiêu đề lấy từ `screenTitles`, kèm nút đóng (×).
- `role="dialog"`, `aria-modal="true"`, `aria-labelledby` trỏ vào tiêu đề.
- Bẫy focus trong popup; `Esc` đóng; đóng xong trả focus về nút đã mở popup. Bấm ra nền **không** đóng popup, để tránh mất nội dung đang gõ.
- Khung popup là phần tử cố định trong `index.html`, ẩn mặc định. Class Tailwind phải viết dưới dạng literal đầy đủ; sáng/tối như các màn khác.

**Mở popup qua hash:**
- `#/queue?open=assign` hoặc `#/queue?open=overview` mở màn Điều phối rồi bật popup tương ứng. Có thể dùng `#/queue/assign` nếu dễ hơn với `parseHash`; ghi cách đã chọn.
- Đóng popup thì đưa hash về `#/queue` bằng `history.replaceState`, không tạo thêm một mục lịch sử.

### 2. Định tuyến và menu
- `parseHash`: `#/` và hash lạ trả về **`queue`**.
- Thêm route **`#/overview`** cho màn Tổng quan đứng riêng.
- Giữ route `#/assign` cho màn Giao việc đứng riêng.
- Hai route trên không có trong menu, chỉ để link sâu và để smoke test truy cập.
- `index.html`: xoá 2 link menu Tổng quan và Giao việc. "Điều phối" đứng đầu menu, trỏ `#/queue`.

### 3. Màn Điều phối
- Trên thanh tiêu đề (cạnh các chip trạng thái) thêm 2 nút:
  - **"Giao việc"** (nút chính, màu indigo): mở popup `assign`.
  - **"Tổng quan"** (nút phụ, viền): mở popup `overview`.
- Ô giao nhanh giữ nguyên. Thêm một link nhỏ "Mở form đầy đủ (chọn đội hình, quy trình…)" để mở popup `assign`.
- **Phím tắt** trên màn Điều phối, chỉ khi focus không nằm trong `input`, `textarea`, `select` hay `[contenteditable]`, và không có popup nào đang mở:
  - `n` mở Giao việc, `o` mở Tổng quan.
  - Ghi chú phím tắt trong `title` của từng nút.

### 4. Link trong các màn cũ
- `overview.html`: 2 link `href="#/assign"` chuyển thành nút gọi `AB.screenModal.open('assign')`. Nếu Tổng quan đang nằm trong popup, đóng popup Tổng quan rồi mở popup Giao việc. Link tới `#/task/{id}` giữ nguyên: bấm vào sẽ đổi màn, và router tự đóng popup.
- `task.html`: 2 link "← Tổng quan" đổi thành "← Điều phối" với `href="#/queue"`.
- **Giao việc thành công trong popup:**
  - Nếu màn Giao việc chuyển sang `#/task/{id}`, giữ nguyên hành vi này (router đóng popup).
  - Nếu ở lại màn (ví dụ khi đưa vào hàng chờ), đóng popup và tải lại dữ liệu màn Điều phối ngay. Dùng một sự kiện `window.dispatchEvent(new CustomEvent('ab:queue-changed'))` để `queue.js` nghe và tải lại. Không để `assign.js` gọi thẳng vào `queue.js`.

### 5. Kiểm tra cũ phải cập nhật cho khớp
- `scripts/dashboard-smoke.sh`: hash của màn `overview` đổi thành `#/overview`. Màn `assign` vẫn là `#/assign`. Thêm `queue` vào danh sách màn chạy được, nếu smoke đã có cách kiểm tra màn đó.
- `scripts/dashboard-js-check.mjs`:
  - Sửa các kiểm tra liên quan tới `parseHash` hay menu, nếu có.
  - Thêm kiểm tra cho cơ chế popup: `open('overview')` rồi `close()` thì timer của popup bị xoá và timer của màn bên dưới vẫn còn; `#/` trả về `queue`; `#/overview` trả về `overview`.
- `tests/dashboard_assets.rs`: chỉ sửa nếu cần. Các file vẫn giữ nguyên, không xoá file nào.

## Phạm vi file
| Được sửa | Ghi chú |
|---|---|
| `assets/dashboard/js/app.js`, `index.html` | Popup, routing, menu |
| `assets/dashboard/js/queue.js`, `screens/queue.html` | Nút, phím tắt, nghe `ab:queue-changed` |
| `assets/dashboard/screens/overview.html`, `js/overview.js` (chỉ khi cần) | Link Giao việc thành nút mở popup |
| `assets/dashboard/js/assign.js` (chỉ để phát `ab:queue-changed` hoặc đóng popup sau khi gửi) | Không đổi logic form |
| `assets/dashboard/screens/task.html` | Link quay về |
| `assets/dashboard/app.css` | Dựng lại bằng `bash scripts/build-css.sh` |
| `scripts/dashboard-smoke.sh`, `scripts/dashboard-js-check.mjs`, `tests/dashboard_assets.rs` (nếu cần) | Cập nhật kiểm tra |

Không sửa `src/**` (không đổi backend), không sửa `board.js`/`settings.js`/`office3d.js`/`office-rooms.js`.

## Kiểm tra
Chạy ở gốc repo:
```bash
cd agent-brain && cargo build --offline --locked 2>&1 | tail -1 && cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"; cd ..
bash scripts/build-css.sh --check
bash scripts/dashboard-ui-lint.sh
node scripts/dashboard-js-check.mjs
node scripts/office3d-place-check.mjs
for f in agent-brain/assets/dashboard/js/*.js; do node --check "$f" || echo "SYNTAX $f"; done
bash scripts/dashboard-smoke.sh overview assign   # nếu smoke cần trình duyệt mà máy không có thì ghi rõ
bash spikes/lib/scan-secrets.sh
```
**BẮT BUỘC kiểm tra trên trình duyệt thật:** lần trước có lỗi cú pháp lọt qua mọi test Node.
- Chạy binary debug với HOME, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `AGENT_BRAIN_DB` và `AGENT_BRAIN_CONFIG` trỏ vào thư mục tạm, chạy `serve --port 0`. Không đụng serve ở cổng 7437.
- Mở bằng `google-chrome --headless=new` qua DevTools protocol (Node 24 có sẵn `WebSocket`).
- Đi lần lượt: `#/`, `#/queue`, mở popup Giao việc, đóng, mở popup Tổng quan, đóng, `#/overview`, `#/assign`.
- Xác nhận không có exception JS và không có `Alpine Expression Error` trong console.
- Chụp màn hình màn Điều phối khi popup đang mở; ghi đường dẫn ảnh vào báo cáo.
- Chỉ dừng các tiến trình Chrome và serve do chính bạn mở, theo PID đã ghi lại.

## Tiêu chí nghiệm thu
1. Menu không còn "Tổng quan" và "Giao việc". Mở dashboard (`#/`) là vào màn Điều phối.
2. Nút "Giao việc" mở popup chứa form đầy đủ. Giao việc thành công thì popup đóng và màn Điều phối cập nhật (hoặc chuyển sang trang task, giống trước đây).
3. Nút "Tổng quan" mở popup chứa các nhóm task. Bấm một task thì sang trang task và popup đóng.
4. `Esc` và nút × đóng popup. Focus nằm trong popup khi mở và trở về nút cũ khi đóng. Phím `n`/`o` hoạt động, nhưng không bắt phím khi đang gõ.
5. Đóng popup thì polling của Tổng quan dừng, còn polling của Điều phối vẫn chạy (có test chứng minh).
6. `#/overview`, `#/assign`, `#/queue?open=assign` đều hoạt động.
7. Trình duyệt thật không có lỗi JS. Mọi test và kiểm tra pass.

## Không được đụng tới
`.planning/` (kể cả file này), `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`, `agent-brain/src/**`, các repo dự án khác. Không dùng `git checkout/restore/stash/reset` lên file bạn không sửa. Không kill hay restart serve ở cổng 7437, không dùng pkill/killall. Không dùng `git add -A`, không dùng `--no-verify`, không push.

## Commit
Một commit: `feat(agent-brain): Điều phối is the home screen; Tổng quan and Giao việc open as popups`, kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối
File đã sửa, hash commit, kết quả nguyên văn các lệnh kiểm tra, cách quản lý phạm vi timer của popup, đường dẫn ảnh chụp, chỗ lệch plan.
