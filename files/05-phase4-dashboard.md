# Phase 4 — Dashboard nhúng trong binary

## 1. Mục tiêu

Thêm giao diện web vào chính binary `agent-brain`: xem task và log theo thời gian thực, giao việc, kill, xem thống kê token. Vẫn là 1 file chạy duy nhất.

Giải quyết: #2 (phân mảnh khi tự build web app), #3 (theo dõi tiến độ trực quan).

## 2. Phạm vi

**Trong phạm vi:** web server Axum, SSE, giao diện HTML + Tailwind + Alpine.js nhúng vào binary, chọn profile trên UI.

**Ngoài phạm vi:** đăng nhập nhiều người dùng, truy cập từ xa qua internet.

## 3. Phân tích

### 3.1 Vì sao nhúng giao diện
- Không cần Node, venv hay cấu hình CORS: UI và API cùng một server, cùng một origin.
- Giao diện đọc dữ liệu từ lõi Phase 2, không viết lại logic.

### 3.2 Bảo mật tối thiểu
Dashboard có thể chạy lệnh trên máy bạn. Bắt buộc:
- Chỉ lắng nghe `127.0.0.1`, không mở ra mạng ngoài.
- Có token truy cập đơn giản (sinh khi khởi động, in ra terminal).

### 3.3 Lưu ý về Tailwind
Tailwind bản CDN tiện cho thử nghiệm nhưng cần internet. Muốn chạy hoàn toàn offline, build sẵn file CSS và nhúng vào binary.

## 4. Thiết kế

### 4.1 Màn hình
| Màn hình | Nội dung |
|---|---|
| Tổng quan | Danh sách task đang chạy / xong / lỗi, bộ chọn profile |
| Chi tiết task | Pipeline Lead → Dev → QA, trạng thái từng bước, log realtime, nút Kill |
| Giao việc | Ô nhập yêu cầu tiếng Việt, chọn dự án (hoặc hiển thị danh sách khi mơ hồ) |
| Thống kê | Token theo ngày, theo profile, theo dự án, theo vai trò |
| Dự án | Danh sách từ `projects_map.json`, sửa alias, nút quét lại |

### 4.2 API
```
GET  /api/projects
POST /api/projects/scan
GET  /api/tasks
POST /api/tasks                 # giao việc
GET  /api/tasks/:id
GET  /api/runs/:id/stream       # SSE
POST /api/runs/:id/kill
GET  /api/stats?profile=work
GET  /api/projects/:id/status   # fast-path cấp 1
```

### 4.3 Luồng log realtime
```
tiến trình agent ─stdout─► stream.rs ─┬─► ghi SQLite (events)
                                      └─► kênh broadcast ─► SSE ─► trình duyệt
```
Khi trình duyệt mở lại trang giữa chừng: đọc events cũ từ SQLite trước, sau đó nối SSE.

## 5. Công việc

- [ ] Thêm Axum, tạo server chỉ nghe localhost + token
- [ ] Viết các endpoint API dùng lại module Phase 2
- [ ] Kênh broadcast từ `stream.rs` sang SSE
- [ ] Dựng 5 màn hình (giao từng màn hình cho agent)
- [ ] Nhúng file tĩnh vào binary
- [ ] Build CSS offline

## 6. Tiêu chí nghiệm thu (test case)

| ID | Kịch bản | Kết quả mong đợi |
|---|---|---|
| P4-01 | Chạy binary | Mở được dashboard, không cần cài thêm gì |
| P4-02 | Truy cập từ máy khác trong mạng LAN | Không truy cập được |
| P4-03 | Truy cập không có token | Bị từ chối |
| P4-04 | Giao việc từ UI | Log hiện realtime, từng bước pipeline đổi trạng thái |
| P4-05 | Reload trang giữa chừng | Thấy log cũ + tiếp tục nhận log mới |
| P4-06 | Bấm Kill | Run dừng, không còn tiến trình sót (như P2-02) |
| P4-07 | Tên dự án mơ hồ trên UI | Hiện danh sách để chọn, không tự chạy |
| P4-08 | Đổi profile | Thống kê và task mới theo đúng profile |
| P4-09 | Tắt mạng internet | Giao diện vẫn hiển thị đúng style |
| P4-10 | So sánh UI với thiết kế (nếu có Figma) | Khớp theo checklist UI của bạn |

## 7. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Lộ quyền chạy lệnh ra mạng | Chỉ localhost + token (P4-02, P4-03) |
| SSE mất kết nối | Trình duyệt tự kết nối lại, đọc bù từ SQLite |
| Giao diện phình to khó bảo trì | Giữ Alpine.js đơn giản, mỗi màn hình một file |
