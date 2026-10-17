# Biên bản nghiệm thu dashboard (Phase 5 / P4-01..P4-10)

> Ghi chú: Bảng này do người dùng điền kết quả sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md).

| ID | Kịch bản | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| P4-01 | Chạy binary | Mở được dashboard, không cần cài thêm gì |  |  |  |
| P4-02 | Truy cập từ máy khác trong mạng LAN | Không truy cập được |  |  |  |
| P4-03 | Truy cập không có token | Bị từ chối |  |  |  |
| P4-04 | Giao việc từ UI | Log hiện realtime, từng bước pipeline đổi trạng thái |  |  |  |
| P4-05 | Reload trang giữa chừng | Thấy log cũ + tiếp tục nhận log mới |  |  |  |
| P4-06 | Bấm Kill | Run dừng, không còn tiến trình sót (như P2-02) |  |  |  |
| P4-07 | Tên dự án mơ hồ trên UI | Hiện danh sách để chọn, không tự chạy |  |  |  |
| P4-08 | Đổi profile | Thống kê và task mới theo đúng profile |  |  |  |
| P4-09 | Tắt mạng internet | Giao diện vẫn hiển thị đúng style |  |  |  |
| P4-10 | So sánh UI với thiết kế (nếu có Figma) | Khớp theo checklist UI của bạn | Không áp dụng |  | D-18: chưa có thiết kế Figma |

## Kiểm tra giao diện (UI-SPEC, backstop)

> Ghi chú: Điền ở 05-13. Mỗi dòng là một mục backstop trong .planning/phases/05-embedded-dashboard/05-UI-SPEC.md (UI Considerations).

| ID | Kiểm tra | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| UI-01 | Lịch sử run và Thống kê với run không có `result` (run bị Kill ở P4-06) | Chi phí chưa biết hiện `—`, không hiện `≈ $0.0000`; token chỉ hiện `—` khi chưa ghi nhận được số nào |  |  |  |
| UI-02 | Chuỗi dài: đường dẫn dự án từ 120 ký tự ở Dự án, nhánh dài ở Chi tiết task, dòng log thô lớn (khoảng 22 KB) trong hộp Dữ liệu thô | Chữ xuống dòng trong thẻ hoặc cuộn trong hộp, trang không tràn ngang |  |  |  |
| UI-03 | Tiêu đề tiếng Việt nhiều dấu chồng (Ế, Ộ, Ữ) ở Tổng quan (2 dòng) và Chi tiết task (3 dòng) | Dấu ở dòng cuối không bị cắt |  |  |  |
| UI-04 | Hàng nhãn ngày của Theo ngày ở 1024 px và 768 px | Nhãn không chồng lên nhau |  |  |  |
| UI-05 | Header ở 768 px và 480 px | Header xuống dòng, không tràn ngang, mọi nút vẫn dùng được |  |  |  |
| UI-06 | Stepper ở 480 px | Ba bước xếp dọc, không có mũi tên |  |  |  |
