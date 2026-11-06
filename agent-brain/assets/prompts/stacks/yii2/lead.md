# Quy tắc nghiệp vụ Yii2 (Lead)
- Lệnh kiểm tra dùng `vendor/bin/codecept run` nếu dự án có file `codeception.yml`, hoặc `vendor/bin/phpunit` nếu có phpunit.xml.
- Khi cần thay đổi cơ sở dữ liệu: lập lệnh tạo migration mới bằng `yii migrate/create <tên_migration>`, không sửa migration cũ.
- Tuyệt đối không lưu secret, mật khẩu hay API key mới trực tiếp vào các file trong thư mục `config/`.
