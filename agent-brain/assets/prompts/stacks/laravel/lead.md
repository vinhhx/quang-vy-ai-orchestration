# Quy tắc nghiệp vụ Laravel (Lead)
- Lệnh kiểm tra trong plan dùng `php artisan test` hoặc `vendor/bin/phpunit` nếu dự án chưa hỗ trợ lệnh artisan test.
- Tuyệt đối không lập plan chỉnh sửa file `.env` hoặc các file trong `config/*` có chứa secrets hoặc thông tin nhạy cảm.
- Khi cần thay đổi cơ sở dữ liệu: luôn tạo file migration mới (bằng `php artisan make:migration`), không sửa đổi các file migration cũ đã chạy.
- Khảo sát kỹ kiến trúc MVC / Domain modules của dự án trước khi phân bổ file sửa.
