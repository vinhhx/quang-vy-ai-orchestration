# Plan: màn Điều phối không phải cuộn: sidebar chỉ icon, sơ đồ 3D chiếm phần chính, 5 cột vào popup "Xem tất cả", thanh thống kê nhỏ bên phải

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Yêu cầu của người dùng (nguyên văn ý)
Không muốn phải cuộn chuột xuống để xử lý việc. Làm lại giao diện:
1. Sidebar trái chỉ hiện icon; di chuột vào thì hiện tooltip chữ.
2. Sơ đồ (Văn phòng 3D) đẩy lên đầu, chiếm hết bề rộng bên phải menu; phần thống kê Điều phối (thanh trạng thái) nằm DƯỚI box 3D.
3. Phòng ban trải ngang: tên ở trên, số lượng ở dưới. Kho lưu chỉ cần con số, không hiện danh sách task gần nhất.
4. Thêm nút "Xem tất cả" mở popup toàn màn hình chứa đủ 5 cột: Hàng chờ, Cần bạn duyệt, Chờ slot, Đang chạy, Dừng.
5. Thêm một sidebar nhỏ bên phải hiển thị số lượng của 5 nhóm đó; bấm vào nhóm nào thì mở popup (4) và đưa nhóm đó vào tầm nhìn.

## Bố cục đích (màn hình ≥ lg, ví dụ 1920×1080; không có thanh cuộn dọc ở trang Điều phối)
```
┌──┬─────────────────────────────────────────────────────────────┬────────┐
│ab│ Văn phòng 3D   [Về góc mặc định] [3D: Bật]                  │Hàng chờ│
│🏠│ ┌BA┐┌Kỹ thuật┐┌QA┐┌Điều phối┐┌Giám đốc┐┌Studio┐┌Nghỉ┐┌Kho lưu┐ │   3    │
│▦ │ │2 ││   1    ││0 ││   0     ││   1    ││  0   ││ 0  ││  12   │ │Cần duyệt│
│📊│ └──┘└────────┘└──┘└─────────┘└────────┘└──────┘└────┘└───────┘ │   1    │
│📁│ ┌─────────────────────────────────────────────────────────────┐│Chờ slot│
│⚙ │ │                  khung 3D (cao theo màn hình)               ││   0    │
│✈ │ │                                                             ││Đang chạy│
│  │ └─────────────────────────────────────────────────────────────┘│   1    │
│  │ Điều phối [Orchestrator: Bật] [Slot 1/2] [Giao việc] [Tổng quan]│ Dừng   │
│👤│ [Xem tất cả]                    (cảnh báo / lỗi nếu có)          │   0    │
│🌓│                                                                 │[Xem tất│
└──┴─────────────────────────────────────────────────────────────┴  cả]  ──┘
```
- Ô "Giao việc nhanh vào hàng chờ" và mục "Gần đây" vẫn còn, nằm dưới thanh trạng thái (có thể phải cuộn mới thấy; đó là phần ít dùng). Mục Gần đây giữ thu gọn mặc định như hiện nay.
- Màn < lg: sidebar trái vẫn chỉ icon (nằm ngang trên cùng như hiện nay ở mobile là chấp nhận được); sidebar phải chuyển thành một hàng 5 ô thống kê nằm ngay dưới box 3D; popup vẫn hoạt động, 5 cột xếp dọc.

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/assets/dashboard/index.html` | Mục A |
| 2 | `agent-brain/assets/dashboard/js/app.js` | Mục A (chỉ phần `shell`: trạng thái menu profile nếu cần) |
| 3 | `agent-brain/assets/dashboard/screens/queue.html` | Mục B, C, D, E |
| 4 | `agent-brain/assets/dashboard/js/queue.js` | Mục C, D, E (thêm getter/method; không đổi logic dữ liệu) |
| 5 | `agent-brain/dashboard-src/input.css` | Chỉ khi cần thêm lớp/tiện ích Tailwind không suy ra được từ HTML |
| 6 | `agent-brain/assets/dashboard/app.css` | SINH LẠI bằng `bash scripts/build-css.sh`, không sửa tay |
| 7 | Test hoặc script kiểm hiện có cần cập nhật vì đổi cấu trúc (grep `queue.html`, `data-queue-visual`, `Hàng chờ` trong `agent-brain/tests/` và `scripts/`), ví dụ `scripts/office3d-place-check.mjs` | Chỉ cập nhật cho khớp cấu trúc mới, không nới kiểm tra |

Không sửa: các màn khác (`screens/board.html`, `stats.html`, …) ngoài việc chúng dùng chung sidebar mới; `js/office3d.js`, `js/office-rooms.js` (trừ khi bắt buộc để canvas co giãn; có `ResizeObserver` sẵn ở `office3d.js:1048`); mọi file Rust; `~/.config/agent-brain/*`; `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác; `.claude/worktrees/`.

## Ràng buộc kỹ thuật (bắt buộc)
- Alpine dùng bản **CSP** (`vendor/alpine.csp.min.js`): trong thuộc tính `x-*`/`@*`/`:*` chỉ gọi tên getter/method/thuộc tính của component, không viết biểu thức JS phức tạp (không arrow function, không gán, không toán tử ba ngôi lồng nhau ngoài mức file hiện có đang dùng). Làm theo đúng cách `queue.html` hiện đang viết. Logic mới đặt trong `queue.js` / `app.js`.
- CSP `style-src 'self'`: không thêm thẻ `<style>` hay `style="..."` tĩnh; style động dùng object như `roomDotStyle` hiện có.
- Không thêm thư viện hay icon font ngoài. Icon là SVG nội tuyến (stroke `currentColor`, 20–24px, `aria-hidden="true"`).
- Mọi `a|button|input|select|textarea` mới phải có `focus-visible:outline-2 …` như hiện có (kiểm bằng `scripts/dashboard-ui-lint.sh`).
- Hỗ trợ sáng/tối như các phần tử hiện có (cặp lớp `dark:`).
- Giữ nguyên MỌI hành vi và handler của 5 cột (huỷ, duyệt, sửa rồi duyệt, từ chối, đổi ưu tiên, xem trong VP, kill…): chỉ DI CHUYỂN markup của lưới 5 cột (hiện ở `queue.html` dòng ~303–804, `<!-- Năm cột điều phối -->` tới hết cột Dừng) vào trong popup, không viết lại.

### A. Sidebar trái chỉ icon (`index.html`, `app.js`)
1. `<aside>`: từ md trở lên rộng cố định `md:w-16` (thay `md:w-64`), căn giữa, padding nhỏ. Tên "agent-brain" thay bằng logo chữ "ab" (có `title`/`aria-label` "agent-brain").
2. Mỗi link nav: chỉ icon SVG + chữ ẩn cho trình đọc màn hình (`sr-only`), `aria-label` = tên màn, và tooltip hiện khi hover/focus: một `<span role="tooltip">` đặt tuyệt đối bên phải icon, ẩn mặc định, hiện bằng `group-hover:` và `group-focus-visible:` (Tailwind), nền tối, chữ trắng, `whitespace-nowrap`, `z-50`. Giữ `:class="navClass(...)"` và `:aria-current`.
   Icon gợi ý: Điều phối = lưới/sơ đồ, Bảng công việc = bảng cột, Thống kê = biểu đồ cột, Dự án = thư mục, Cài đặt = bánh răng, Telegram = máy bay giấy.
3. Phần dưới sidebar:
   - Bộ lọc profile: nút icon (người) có tooltip `"Profile: <giá trị hiện tại>"`; bấm mở menu nhỏ (popover, bên phải) gồm 3 lựa chọn Tất cả / work / personal; chọn → gọi đúng luồng `profileChanged` hiện có. Esc hoặc bấm ra ngoài đóng menu. Giữ `title` giải thích như select cũ. Nếu làm popover phức tạp với Alpine CSP, có thể giữ `<select>` nhưng thu nhỏ trong popover; không bỏ chức năng.
   - Nút đổi giao diện sáng/tối: chỉ icon (mặt trời/mặt trăng), `aria-label` = `themeAria`, tooltip = `themeLabel`.
4. Mobile (< md) giữ hàng ngang như hiện nay, nhưng chỉ icon.

### B. Sơ đồ 3D lên đầu, thanh trạng thái xuống dưới (`queue.html`)
1. Thứ tự mới trong `<section data-screen="queue">`: (1) khối 3D `data-queue-visual`; (2) thanh trạng thái (`<h1>Điều phối</h1>`, badge Orchestrator, slot, Giao việc, Tổng quan, nút mới "Xem tất cả", dòng auto-resume); (3) cảnh báo Orchestrator tắt và lỗi API; (4) Giao việc nhanh; (5) Gần đây.
2. Bọc màn trong lưới hai cột ở ≥ lg: cột chính (3D + phần dưới) và cột phải hẹp `w-28` cho sidebar thống kê (Mục E). Cột phải `sticky top-0` cao bằng màn hình.
3. Khung 3D: bỏ chiều cao cố định `h-80 lg:h-[520px]`; dùng chiều cao theo màn hình để khung 3D + hàng phòng ban + thanh trạng thái vừa một màn hình 1080p không cuộn, ví dụ `h-[50vh] lg:h-[calc(100vh-17rem)] min-h-80`. Đo thật ở 1920×1080 và 1366×768; chỉnh số rem cho đúng và ghi giá trị cuối trong tóm tắt. Canvas phải co giãn theo (ResizeObserver có sẵn).
4. `<main id="screen">` hiện có `px-6 py-8`: cho màn Điều phối giảm padding dọc (ví dụ `py-4`) nếu cần để không cuộn; không đổi padding các màn khác (dùng lớp trên section của queue, không sửa `<main>`).

### C. Phòng ban trải ngang (`queue.html`, `queue.js`)
1. Bỏ cây phòng ban dọc bên trái khung 3D (`<aside class="hidden lg:flex flex-col w-64 …">`, dòng ~174–209). Thay bằng một hàng ngang NẰM TRÊN khung 3D, trong cùng box `data-queue-visual`.
2. Mỗi phòng là một nút (`@click="focusRoom(r.id)"` như cũ): dòng trên là chấm màu (`roomDotStyle(r)`) + tên phòng; dòng dưới là số lượng (`r.items.length`) cỡ chữ lớn, đậm. Các nút chia đều bề ngang (`grid grid-cols-4 sm:grid-cols-8` hoặc `flex` + `flex-1`), không cuộn ngang ở ≥ lg. Phòng đang được chọn (nếu có trạng thái) được tô nổi.
3. KHÔNG hiện danh sách task trong bất kỳ phòng nào ở hàng này (gồm Kho lưu): chỉ tên + số. Việc mở từng task vẫn làm qua nhân vật trong 3D và thẻ chi tiết nổi hiện có.
4. Giữ con số tổng `totalOfficeCount + ' nhân vật'` ở thanh điều khiển box 3D.

### D. Popup "Xem tất cả" với 5 cột (`queue.html`, `queue.js`)
1. `queue.js`: thêm trạng thái `allOpen: false`, `allFocus: ''` và method `openAll(focus)` (focus ∈ `'intake'|'approval'|'slot'|'running'|'stopped'|''`), `closeAll()`, getter phục vụ class/aria nếu cần. `openAll` đặt `allOpen = true`, rồi sau khi render (`this.$nextTick`) cuộn cột có `data-col="<focus>"` vào tầm nhìn (`scrollIntoView({block:'nearest', inline:'start'})`) và đặt focus vào tiêu đề cột đó (`tabindex="-1"`). Với Alpine CSP, gọi bằng các method không tham số nếu cần: `openAllIntake()`, `openAllApproval()`, … mỗi cái gọi `openAll('…')`.
2. `queue.html`: một overlay `fixed inset-0 z-40` (dưới modal chung `#ab-screen-modal` z-50 để popup Giao việc/Tổng quan vẫn mở được bên trên), `x-show="allOpen"`, `role="dialog" aria-modal="true" aria-labelledby="ab-all-title"`, nền mờ giống modal chung. Bên trong: thanh tiêu đề "Tất cả việc" + nút đóng (×, `aria-label="Đóng"`), thân cuộn được (`overflow-auto`) chứa NGUYÊN lưới 5 cột được di chuyển từ trang vào (giữ class `grid … xl:grid-cols-5`), mỗi cột thêm `data-col="intake|approval|slot|running|stopped"`. Cột được focus có viền nhấn (ring indigo) trong lúc popup mở.
3. Đóng bằng: nút ×, phím Esc (thêm vào `keyHandler` hiện có: Esc khi `allOpen` → `closeAll()`; không chặn Esc của modal chung khi nó đang mở), bấm vào nền mờ bên ngoài panel. Khi đóng, trả focus về nút đã mở popup.
4. Phím tắt mới: `a` mở popup (cùng điều kiện bỏ qua khi đang gõ trong input như `n`/`o`); ghi vào `title` của nút "Xem tất cả" là `"Xem tất cả (a)"`.
5. Nút "Xem tất cả" đặt ở thanh trạng thái (Mục B) và ở cuối sidebar phải (Mục E).

### E. Sidebar thống kê bên phải (`queue.html`, `queue.js`)
1. Năm ô xếp dọc, mỗi ô là một `<button>`: nhãn trên (Hàng chờ / Cần duyệt / Chờ slot / Đang chạy / Dừng), số dưới (to, đậm), màu nhấn theo nhóm (ví dụ Dừng màu rose khi > 0, Cần duyệt màu amber khi > 0, Đang chạy màu indigo). `@click` mở popup với cột tương ứng (Mục D1). `aria-label` = `"<Nhãn>: <số> — mở danh sách"`.
2. Số lấy từ getter có sẵn: `intakeItems.length`, `approvalItems.length`, `waitingSlotItems.length`, `runningTasks.length`, `stoppedTasks.length`. Với Alpine CSP, thêm getter `countIntake`, `countApproval`, `countSlot`, `countRunning`, `countStopped` trong `queue.js` thay vì viết `.length` trong HTML nếu bản CSP không cho.
3. Cuối cột: nút "Xem tất cả" (mở popup không focus cột nào).
4. ≥ lg: cột dọc `sticky top-0 h-screen` bên phải. < lg: một hàng 5 ô ngang (`grid grid-cols-5`) đặt ngay dưới box 3D.

## Không được đụng tới
- Logic dữ liệu, API, polling, SSE của `queue.js` (chỉ thêm trạng thái UI và getter/method).
- Hành vi các nút trong 5 cột.
- Modal chung `#ab-screen-modal` và luồng Giao việc / Tổng quan.
- Không `git add -A`, không `--no-verify`.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration
bash scripts/build-css.sh            # sinh lại app.css
bash scripts/build-css.sh --check    # app.css khớp nguồn
bash scripts/dashboard-ui-lint.sh    # mọi thẻ tương tác có focus-visible
node scripts/dashboard-js-check.mjs  # cú pháp JS dashboard (xem cách gọi trong file)
node scripts/dashboard-keys.mjs      # nếu script này kiểm phím tắt (đọc file trước)
cd agent-brain && cargo test --no-fail-fast   # gồm test assets dashboard (dashboard_assets.rs…)
git diff --stat <HEAD trước khi chạy>..HEAD   # chỉ các file trong bảng Thay đổi
```
Kiểm bằng trình duyệt (bắt buộc, ghi kết quả vào tóm tắt): chạy một `agent-brain serve` THỬ trên cổng khác với dữ liệu tạm (KHÔNG dừng hay đụng serve đang chạy của người dùng ở cổng 7437; đọc `agent-brain serve --help` để đặt cổng và thư mục dữ liệu/config tạm, ví dụ biến môi trường XDG trỏ vào thư mục tạm). Mở bằng Chrome headless hoặc công cụ có sẵn, chụp ảnh ở 1920×1080 và 1366×768, sáng và tối, và xác nhận:
- trang Điều phối không có thanh cuộn dọc ở 1920×1080 cho phần 3D + phòng ban + thanh trạng thái + sidebar phải;
- tooltip sidebar hiện khi hover và khi focus bằng Tab;
- bấm từng ô thống kê mở popup và cột đúng được cuộn vào tầm nhìn; Esc / × / bấm nền đóng popup;
- phím `n`, `o`, `a` hoạt động; khi gõ trong ô nhập thì không kích hoạt.
Lưu ảnh chụp vào thư mục scratchpad/tạm và ghi đường dẫn trong tóm tắt (không commit ảnh).

## Tiêu chí hoàn thành
1. Năm yêu cầu của người dùng đều có, đúng mô tả ở trên.
2. Không mất chức năng nào của 5 cột, sidebar, profile, theme, Giao việc, Tổng quan.
3. Các lệnh kiểm tra pass; diff chỉ trong các file của bảng.

## Commit
Mục A một commit; B+C một commit; D+E một commit; app.css đi cùng commit làm đổi lớp. Message `feat(dashboard): …`, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt cuối: file sửa, commit, chỗ lệch plan, kết quả từng lệnh kiểm tra, đường dẫn ảnh chụp, phần chưa kiểm.
