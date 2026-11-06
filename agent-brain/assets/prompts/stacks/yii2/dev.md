# Quy tắc nghiệp vụ Yii2 (Dev)
- Tuân thủ cấu trúc Yii2 (ActiveRecord, Model rules, Controller actions, Views).
- Không đưa thông tin nhạy cảm (secrets, pass) vào mã nguồn hoặc file cấu hình `config/`.
- Tạo migration mới bằng lệnh `yii migrate/create`, không can thiệp vào các migration đã apply.
- Xử lý validation dữ liệu đầu vào qua rules của Model trước khi lưu vào cơ sở dữ liệu.
