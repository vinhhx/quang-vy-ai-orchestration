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
9. **Dev hết giờ (60 phút) giữa chừng mà không commit gì, và không dọn dữ liệu.**
   - Task #16: 25 TC, mỗi lần chạy đầy đủ mất 17 phút (Livewire chậm), Dev chạy vài lần thì `timed_out`. File viết dở vẫn nằm trong thư mục làm việc, không có commit, không có `test-results.json`. Các bản ghi `E2E-` còn sót trên site dev.
   - Gợi ý sửa agent-brain:
     - (a) cảnh báo hoặc gửi tín hiệu cho Dev khi còn khoảng 10 phút (ví dụ qua file `run_dir/deadline` hoặc nhắc trong prompt "bạn có N phút"), để Dev kịp chốt kết quả;
     - (b) timeout theo dự án hoặc quy trình (e2e cần dài hơn);
     - (c) khi Dev `timed_out` mà file trong plan đã đổi, autopilot có thể chạy một lượt "chốt" ngắn: chỉ ghi kết quả và commit.
   - Về cách làm (ghi chú prompt e2e): giới hạn khoảng 12–15 TC mỗi task để một lần chạy chỉ mất khoảng 6–8 phút.
10. **Điều phối viên (tôi) duyệt nhầm một plan chưa chứa phần sửa.** Edit thất bại nhưng lệnh commit và duyệt vẫn chạy trong cùng chuỗi. Bài học cho script điều phối: kiểm nội dung plan (grep mục mới) trước khi duyệt. agent-brain cũng nên hiện diff plan so với lần duyệt trước khi bấm Duyệt.
11. **Lệnh Bash của Dev bị cắt sau 10 phút rồi chuyển ra nền; Dev đọc nhầm kết quả.**
    - Task #16 lần Dev 2: một lần chạy đầy đủ (~20 phút) vượt giới hạn 600 giây của Bash, bị đẩy ra nền. Lần sau bị ngắt (exit 130/137). Dev kết luận sai là "thiếu DISPLAY" và ghi 25 ca `blocked`.
    - Gợi ý agent-brain: prompt Dev nên nói rõ giới hạn 10 phút/lệnh và không được dùng lệnh chạy nền trong `-p`; QA nên phát hiện kết luận "blocked toàn bộ" mâu thuẫn với log.
    - Lỗi của điều phối viên (tôi): plan ghi "chạy riêng ca bằng `--case TC-xx`", nhưng `--case` của `run-task.mjs` chỉ in trace. Lọc ca thật phải dùng `-- --grep`. Bài học: kiểm `--help` của công cụ trước khi viết vào plan.
12. **Ngân sách workflow tính cả các lần Dev hết giờ.** 120 phút bị hết sau 2 lần Dev 60 phút, nên autopilot dừng ngay sau khi duyệt lại plan (job 6), không chạy Dev. Thông báo chỉ có trên Telegram và log serve; API duyệt vẫn trả `job_id` như thể sẽ chạy. Gợi ý: kiểm ngân sách lúc duyệt và trả lỗi rõ ràng, hoặc cho nút "gia hạn ngân sách". Hiện đã tăng `e2e-tu-dong.budget_minutes` lên 200.
13. **Vướng mắc đã xử lý vẫn chặn autopilot; phải xóa tay.** Sau khi Dev ghi Vướng mắc, điều phối viên thêm mục hướng dẫn rồi duyệt lại. Nhưng autopilot vẫn dừng ("Vướng mắc chưa xử lý trong .plan.md") vì mục Vướng mắc còn chữ. Gợi ý: cho đánh dấu "đã xử lý" (ví dụ checkbox `- [x]`), hoặc khi duyệt lại thì hỏi có xóa vướng mắc cũ không. API duyệt cũng nên báo lỗi ngay thay vì trả `job_id`, giống mục 12.
14. (Dự án e2e, không phải agent-brain) Runner nạp mọi `*.e2e.ts` của project theo `tests` glob, nên hai task cùng tên session `admin` gây `COLLECTION_ERROR`. Đã đổi sang tên session riêng mỗi task (`admin-<task>`). Mẫu task / RULES.md của e2e nên ghi luật này.
15. **Số vòng sửa của autopilot không khớp cấu hình.** Workflow `e2e-tu-dong` có `max_rounds: 2`, nhưng task #17 dừng với "QA vẫn kết luận Cần sửa sau 1 / 1 vòng". Có thể lần Dev bị `dev:blocked` đã bị tính một vòng, hoặc `max_rounds` không được đọc. Cần kiểm lại cách đếm. Lần Dev blocked vì vướng mắc không nên tính là một vòng QA.
16. (Dự án e2e) `app.screenshot` bị `POLICY_DENIED` sau khi session đã điền mật khẩu (bất biến bảo mật). Mọi task dùng đăng nhập sẽ không có ảnh lỗi. Bằng chứng thay thế là `screen-at-failure.txt` và `trace.md`. Mẫu plan và ghi chú lead nên dùng tiêu chí này.
17. **(Review Plan 1b) Đề xuất trong chuỗi bị đánh dấu thất bại quá sớm.** `scheduler.rs` đánh dấu đề xuất kế tiếp thất bại khi run **gần nhất** của task trước là `timed_out`/`failed`, kể cả khi task chưa kết thúc và còn được chạy lại (như task #16: Dev hết giờ 2 lần rồi lần 3 xong). Nên chỉ dựa vào trạng thái cuối của **task**. Ngoài ra còn một khối `if prior_p.status == "rejected" || …` bị lặp, là code chết.
