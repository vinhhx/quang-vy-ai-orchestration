# Quy tắc nghiệp vụ Laravel (QA)
- Chạy các unit/feature test liên quan đến tính năng vừa sửa: `php artisan test --filter <TênTest>` hoặc `vendor/bin/phpunit tests/...`.
- Kiểm tra mã lỗi HTTP, format JSON phản hồi nếu sửa API endpoint.
- Đảm bảo không có lỗi cú pháp PHP (chạy `php -l` cho các file đã sửa nếu cần).
