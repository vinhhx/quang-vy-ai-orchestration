# Plan: các nút Duyệt / Sửa rồi duyệt / Từ chối / Huỷ / đổi ưu tiên trên màn Điều phối bị khoá ngay khi tải (Alpine CSP và truy cập theo khoá động)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main` (đã có giao diện mới c795aed). Đọc toàn bộ file này trước khi làm.

## Sự cố (executor của plan giao diện phát hiện, đã kiểm cả trên base 3bf876f)
- Trên màn Điều phối, các nút "Duyệt", "Sửa rồi duyệt", "Từ chối" (thẻ đề xuất, Cần duyệt), "Huỷ" (Hàng chờ), đổi ưu tiên (Chờ slot), và 3 nút trong thẻ chi tiết nổi của Văn phòng 3D render ở trạng thái **disabled** ngay khi tải, dù `actionBusy` là `{}`.
- Console có cảnh báo `Alpine Expression Error … editForms[item.data.id].title` và tương tự với `rejectForms`.
- Mọi chỗ dùng dạng truy cập theo khoá động trong thuộc tính Alpine nằm ở `agent-brain/assets/dashboard/screens/queue.html`:
  - `:disabled="actionBusy[…id]"` (dòng ~104, 110, 116, 432, 541, 547, 553, 762)
  - `x-show`/`x-text="cardErrors[…id]"` (~439, 441, 700, 702, 781, 783)
  - toàn bộ form "Sửa rồi duyệt" `editForms[item.data.id]…` (~566–651, gồm `x-model`)
  - form "Từ chối" `rejectForms[item.data.id]…` (~658–693, gồm `x-model`)
- Dashboard dùng bản Alpine **CSP** (`vendor/alpine.csp.min.js` 3.17.4). Giả thuyết: bộ phân tích biểu thức của bản CSP không xử lý đúng truy cập thành viên tính toán `obj[expr]` hoặc `obj[expr].prop` (trả giá trị sai/ném lỗi), nên `:disabled` thành thuộc tính có mặt. Bước 1 phải XÁC NHẬN nguyên nhân thật trước khi sửa.

## Mục tiêu
1. Xác nhận nguyên nhân bằng một trang thử nhỏ (ngoài repo, trong scratchpad) nạp `vendor/alpine.csp.min.js` với các biểu thức `actionBusy[it.id]`, `cardErrors[it.id]`, `editForms[id].title` và ghi kết quả (giá trị trả về, lỗi console) vào tóm tắt.
2. Sửa sao cho không còn biểu thức truy cập theo khoá động nào trong thuộc tính Alpine của `queue.html`:
   - `queue.js`: thêm method `isBusy(id)` → `!!this.actionBusy[id]`, `cardError(id)` → chuỗi lỗi hoặc `''`; thay mọi `:disabled="actionBusy[…]"` bằng `:disabled="isBusy(…)"`, mọi `cardErrors[…]` bằng `cardError(…)`.
   - Form Sửa rồi duyệt và Từ chối: chọn MỘT cách hoạt động được với Alpine CSP (kể cả `x-model`) và ghi lý do trong tóm tắt, ví dụ:
     (a) mỗi thẻ đề xuất dùng một component con `x-data="proposalForm"` (đăng ký bằng `Alpine.data` trong `queue.js`) giữ `editOpen/title/team/workflow/priority/sending/error` và `rejectOpen/reason/…` cục bộ, gọi các method của component cha qua `$dispatch` hoặc một hàm dùng chung; hoặc
     (b) gắn sẵn object form vào từng phần tử (ví dụ `item.form`) khi dựng `approvalItems`, rồi bind `x-model="item.form.title"` (đường dẫn thuộc tính tĩnh).
     Giữ nguyên hành vi: mở/đóng form, Esc đóng, giá trị mặc định từ đề xuất, gửi duyệt/từ chối, hiện lỗi, trạng thái đang gửi, focus.
3. Không còn `Alpine Expression Error` nào trên console của màn Điều phối khi tải và khi thao tác các form.
4. Rà các màn khác (`screens/*.html`) theo cùng mẫu (`[a-zA-Z_]\[[a-zA-Z_.]+\]` trong thuộc tính `x-*`, `:*`, `@*`): hiện chỉ `queue.html` có; nếu tìm thấy ở màn khác thì ghi vào tóm tắt, KHÔNG sửa (ngoài phạm vi).

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/assets/dashboard/screens/queue.html` | Mục tiêu 2 |
| 2 | `agent-brain/assets/dashboard/js/queue.js` | Mục tiêu 2 (method/component mới; không đổi logic API) |
| 3 | `agent-brain/assets/dashboard/app.css` | Sinh lại bằng `bash scripts/build-css.sh` chỉ khi lớp CSS đổi |
| 4 | `scripts/dashboard-js-check.mjs` | Chỉ khi cần cập nhật kiểm tra cho khớp, hoặc THÊM một kiểm tra chặn mẫu `obj[expr]` trong thuộc tính Alpine của `screens/*.html` (khuyến nghị) |

Không sửa: file Rust, các màn khác, `office3d.js`, `office-rooms.js`, `vendor/*`, `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác.

## Ràng buộc
- Alpine CSP: không biểu thức phức tạp trong thuộc tính; logic trong `queue.js`. Không `<style>`/`style=` tĩnh. Mọi nút giữ `focus-visible:outline-2 …`.
- Serve thật của người dùng (cổng 7437) không được dừng hay đụng. Kiểm bằng serve thử cổng khác với HOME/XDG/`AGENT_BRAIN_DB`/`AGENT_BRAIN_CONFIG` tạm, như plan `queue-screen-no-scroll-layout`; chỉ dừng PID mình khởi chạy, không `pkill`/`killall`.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration
bash scripts/build-css.sh --check
bash scripts/dashboard-ui-lint.sh
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-smoke.sh queue --keys
cd agent-brain && cargo test --no-fail-fast
git diff --stat <base>..HEAD
```
Trình duyệt (bắt buộc, dữ liệu giả có ít nhất 1 đề xuất pending, 1 task chờ duyệt plan, 1 intake queued, 1 mục chờ slot):
- khi tải: các nút Duyệt / Sửa rồi duyệt / Từ chối / Huỷ / đổi ưu tiên KHÔNG disabled; console không có `Alpine Expression Error`;
- bấm Duyệt → gọi đúng API (chặn request bằng Chrome để kiểm body), trong lúc chờ nút disabled, xong thì bật lại;
- Sửa rồi duyệt: mở form, sửa tiêu đề/đội/workflow/ưu tiên, Lưu & Duyệt gửi đúng giá trị; Esc đóng; lỗi API hiện trong form;
- Từ chối: nhập lý do, xác nhận gửi đúng; lỗi hiện đúng chỗ;
- Huỷ intake và đổi ưu tiên chờ slot gọi đúng API.
Chụp ảnh trước/sau (thẻ đề xuất với nút bật) và ghi đường dẫn tuyệt đối.

## Commit
Một commit `fix(dashboard): queue action buttons and forms work under Alpine CSP`, add đúng file, không `--no-verify`, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt: nguyên nhân đã xác nhận, cách sửa form đã chọn và lý do, file sửa, kết quả từng kiểm tra, ảnh chụp.
