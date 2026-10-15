# Biên bản nghiệm thu giao thức MVP (Phase 1 / P1-01..P1-10)

> Ghi chú: Bảng này do người dùng điền kết quả sau khi thực hiện các bước trong [protocol-manual.md](../tests/protocol-manual.md).

| ID | Kịch bản | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
|---|---|---|---|---|---|
| P1-01 | Gọi bằng tên có dấu "web bán hàng" | Khớp đúng `web-ban-hang` | chưa chạy | | |
| P1-02 | Gọi bằng alias "shop" | Khớp đúng | chưa chạy | | |
| P1-03 | Tên mơ hồ khớp 2 dự án | Dừng, liệt kê 2 dự án, không chạy agent | chưa chạy | | |
| P1-04 | Tên không khớp dự án nào | Dừng, liệt kê toàn bộ, không chạy agent | chưa chạy | | |
| P1-05 | Clone dự án mới rồi chạy quét | Dự án mới xuất hiện, alias cũ không bị mất | chưa chạy | | |
| P1-06 | Lead chạy xong | Không có file code nào bị sửa (`git status` sạch ngoài `.plan.md`) | chưa chạy | | |
| P1-07 | Dev chạy với plan đầy đủ | Chỉ file trong plan bị sửa | chưa chạy | | |
| P1-08 | Dev chạy với plan thiếu thông tin | Ghi Vướng mắc, không sửa bừa | chưa chạy | | |
| P1-09 | QA chạy | Có `.qa-report.md`, không sửa code | chưa chạy | | |
| P1-10 | Dự án profile `work` | Chạy đúng `CLAUDE_CONFIG_DIR` của work | chưa chạy | | |
