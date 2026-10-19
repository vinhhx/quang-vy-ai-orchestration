# Biên bản nghiệm thu dashboard (Phase 5 / P4-01..P4-10)

> Ghi chú: Bảng này do người dùng điền kết quả sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md).

| ID | Kịch bản | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| P4-01 | Chạy binary | Mở được dashboard, không cần cài thêm gì | Đạt | 2026-10-19 | Claude chạy (theo yêu cầu): binary musl mới cài vào ~/.local/bin (sha256 f39ca585…); `agent-brain serve` với cấu hình thật; 5 màn hình Tổng quan, Giao việc, Chi tiết task (task 7), Thống kê, Dự án render đủ (Chrome headless, data-loaded=yes). Bản dựng release cũng qua P4-01 trong env -i. |
| P4-02 | Truy cập từ máy khác trong mạng LAN | Không truy cập được | Đạt | 2026-10-19 | Máy khác trong LAN: `curl http://192.168.42.158:7437/` → Couldn't connect (người dùng, 2026-10-19). Đối chứng: máy LAN đó cũng không vào được cổng 8000 của máy này (đang nghe 0.0.0.0), nên giữa hai máy có firewall/cách ly mạng — phép thử chéo máy không tách được nguyên nhân. Bằng chứng ràng buộc loopback (Claude, khi serve đang chạy): `ss -ltn` chỉ có 127.0.0.1:7437; từ chính máy này qua IP LAN 192.168.42.158: cổng 8000 → 200, cổng 7437 → từ chối (rc=7); qua 127.0.0.1:7437 → 401. Người dùng chọn ghi Đạt theo bằng chứng đối chứng tại chỗ. |
| P4-03 | Truy cập không có token | Bị từ chối | Đạt | 2026-10-19 | Claude chạy (curl): không cookie / cookie sai / ?token sai → 401 ở /, /app.css, /api/tasks, SSE; token đúng → 303 rồi 200; Host lạ → 421; POST Origin lạ → 403. Sau Ctrl+C (SIGINT theo PID) và khởi động lại: token mới khác token cũ, cookie cũ → 401, URL mới chạy. Kiểm lại lần 2 (2026-10-19, theo yêu cầu): sau lần khởi động lại thứ hai cookie cũ và ?token cũ → 401 ở /, /app.css, /api/tasks, SSE; URL mới → 303 rồi 200. |
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
| UI-04 | Hàng nhãn ngày của Theo ngày ở 1024 px và 768 px | Nhãn không chồng lên nhau | Đạt | 2026-10-19 | Claude (Chrome headless 1024 và 768 px, Tất cả/30 ngày): dữ liệu thật mới có 1 ngày (16/10) nên chỉ 1 nhãn — không chồng; cần kiểm lại khi có trên 14 ngày dữ liệu. |
| UI-05 | Header ở 768 px và 480 px | Header xuống dòng, không tràn ngang, mọi nút vẫn dùng được | Đạt | 2026-10-19 | Claude (Chrome headless 768 và 480 px): header xuống dòng, nav, Lọc theo profile và nút Giao diện vẫn hiện đủ. Phát hiện Tổng quan tràn ngang ở 480 px (slug dài trong thẻ nhóm) — đã sửa bằng fix plan 05-08-overview-narrow-overflow (agy, commit 77e94ed), chụp lại: hết tràn. |
| UI-06 | Stepper ở 480 px | Ba bước xếp dọc, không có mũi tên | Đạt | 2026-10-19 | Claude (Chrome headless 480 px, task 7): ba bước Lead, Dev, QA xếp dọc, không có mũi tên. |
