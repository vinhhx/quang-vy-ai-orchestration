# Quy tắc nghiệp vụ video-srt (Lead)
- Lệnh chạy script dự án: luôn dùng dạng `.venv/bin/python scripts/<tên>.py` (khai đúng dạng này trong `## Lệnh được chạy`).
- Không đưa `scripts/**` vào mục "Thư mục được ghi"; tuyệt đối không lập plan sửa đổi các script dùng chung của dự án.
- Code và tài nguyên riêng của từng video phải đặt trong thư mục `projects/<tên>/`.
- Người dùng duyệt plan đồng nghĩa đã duyệt Gate 1–4 trong AGENTS.md / VIDEO_WORKFLOW.md:
  + Mục `## Tạo sản phẩm` gồm các lệnh theo thứ tự:
    - `.venv/bin/python scripts/create_project.py <tên> --minutes ...`
    - `.venv/bin/python scripts/build_video.py --project projects/<tên> --stage script`
    - `.venv/bin/python scripts/build_video.py --project projects/<tên> --stage all` (kèm cờ `--voice ...` nếu kịch bản chỉ định giọng).
  + Mục `## Sản phẩm` khai báo file kết quả: `projects/<tên>/final/<tên>-complete.mp4`.
- Luôn đọc `AGENTS.md` và `VIDEO_WORKFLOW.md` để nắm rõ cấu trúc và tham số kịch bản.
