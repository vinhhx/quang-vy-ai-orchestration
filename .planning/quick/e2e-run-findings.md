# Nhật ký lỗi quy trình agent-brain khi chạy e2e toàn bộ apec (2026-11-07)

Ghi lại khi chạy thật, để gộp vào một plan sửa sau. Các mục đã có trong `task-handoff.plan.md` §5b thì chỉ dẫn chiếu.

## Đã ghi trong task-handoff §5b (Plan 1, đang chạy)
- Đề xuất không ghi `task_id` (§5b.1).
- Đã duyệt nhưng vẫn hiện "Chờ duyệt plan"; quy trình không auto không có hướng dẫn chạy Dev (§5b.2).
- Tin Telegram cũ sau khi plan được commit lại (§5b.3).
- Nút Chạy Dev bị khóa không rõ lý do; grant sai chỉ phát hiện sau khi duyệt (§5b.4).
- QA e2e chạy lại test làm đổi `results/` nên bị gắn vi phạm giao thức (§5b.5).

## Mới
1. **Độ khó của đề xuất không áp vào task, nên chọn sai thành viên.**
   - Task #16: đề xuất `level = kho`, nhưng `tasks.level = vua`, nên Lead chạy `kh-sonnet` (Dễ/Vừa) thay vì `kh-opus` (Khó).
   - Nguyên nhân 1: `scheduler::schedule_tick` gọi `set_task_level` chỉ khi tra được `task_id`, mà phép tra đó luôn trả về None (§5b.1).
   - Nguyên nhân 2: kể cả khi tra được, level vẫn được đặt **sau** `start_new_task`, tức là sau khi Lead đầu tiên đã được chọn thành viên.
   - Sửa: truyền `level` vào `StartRequest` và ghi vào task trước khi chạy Lead.
2. **Lead tự sửa plan (`plan_invalid`, lần 2/2) nhưng lý do không hiện ở đâu.** Lý do chỉ in ra stdout của serve. Nên lưu danh sách lỗi vào `runs.error` hoặc `metrics_json` để dashboard hiện.
3. **Triage tách một yêu cầu thành nhiều đề xuất** (có thể xảy ra, tối đa 5). Khi người gửi đã chỉ rõ dự án và phạm vi, nên có tùy chọn "không tách". Hiện đang xử lý bằng cách từ chối các đề xuất thừa.
4. **Không tạo được task trực tiếp khi orchestrator bật.** `POST /api/tasks` luôn đi qua intake → triage. Triage tốn một lượt model và có thể đổi lại đội hình / quy trình. Cần tùy chọn "giao thẳng" với dự án, quy trình, đội hình, độ khó cố định.
5. **Lead tin rằng Dev không đọc được repo nguồn.** Plan task #16 ghi "Dev KHÔNG đọc được repo apec", dù Dev có `--add-dir` và block "Dự án nguồn". Block nguồn nên nói rõ "mọi vai của task (lead, dev, qa…) đều đọc được".
6. **Plan e2e đặt luật quá chặt về số lần chạy lại** ("chạy đúng 1 lần, chờ người dùng xác nhận"), khiến Dev có thể kẹt. Ghi chú prompt của dự án nên nói rõ: mỗi lần chạy 1 lần đăng nhập, được chạy lại tối đa 5 lần.
