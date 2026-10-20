# Biên bản nghiệm thu Phase 5.2 — Quy trình và Autopilot (AUTO-07)

> Ghi chú: Bảng này được điền sau khi thực hiện các bước nghiệm thu trong [LIVE-CHECKS.md](../agent-brain/LIVE-CHECKS.md), mục `4. Nghiệm thu Phase 5.2 (AUTO-07)`. Mọi ca chạy với `claude` thật trên `/mnt/data/IDE/du-an-nhap` (profile `work`), trong môi trường cô lập của P5.2-00 (bản sao `brain.db`, cấu hình riêng, dashboard cổng 7438). Id run, vai trò, giai đoạn, vòng, kết luận và chi phí được chép nguyên văn từ câu SQL của từng ca, không làm tròn.

## Môi trường (P5.2-00)

- Commit của repo (binary release):
- `claude --version`:
- `user_version` của bản sao trước / sau `stats --json`:
- Số dòng trước (projects / tasks / runs / events):
- Số dòng sau (projects / tasks / runs / events):
- Ngày:

| ID | Kịch bản | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| P5.2-01 | `p52-full` (mac-dinh, ngân sách $5) tới QA đạt, có ít nhất một vòng sửa lại plan do lỗi cài vào plan | Review plan `review:revise` ở vòng 1, Lead vòng 2 với `--review`, Review plan `review:approve`, autopilot dừng ở cổng duyệt; sau `approve`: Code → Review code → QA `qa:pass`, `auto_state = done`, thoát `0` |  |  |  |
| P5.2-02 | Dừng do ngân sách: `auto --start --workflow chi-phi-thap` (ngân sách $0.05) | Một dòng `Autopilot dừng · hết ngân sách: …`, thoát `4`, `auto_json.stop.reason = het_ngan_sach`, chỉ một run Lead, không có run `plan-review` |  |  |  |
| P5.2-03 | Kill giữa vòng lặp: `agent-brain kill` khi Review plan đang chạy | Run `killed`, `auto_state = stopped`, lý do `dung_boi_ban`, không còn run `running`; sau 5 giây `ps -p` các PID đã ghi chỉ in dòng tiêu đề |  |  |  |
| P5.2-04 | Giao diện trên dữ liệu thật (dashboard cổng 7438), 1024 / 768 / 480 px, sáng và tối | Sáu mục UI52-01 … UI52-06 bên dưới đều đạt |  |  |  |

## Kiểm tra giao diện (UI-SPEC 05.2, backstop)

> Ghi chú: Mỗi dòng là một mục backstop (🧪) trong `.planning/phases/05.2-workflows-and-autopilot/05.2-UI-SPEC.md` (UI Considerations). Ghi bề rộng, giao diện sáng / tối và task đã xem.

| ID | Kiểm tra | Kết quả mong đợi | Kết quả | Ngày | Ghi chú |
| --- | --- | --- | --- | --- | --- |
| UI52-01 | Luồng pipeline ở 1024 px và 768 px (task `p52-loop`, đủ các nhóm của mac-dinh) | Luồng là một hàng; dưới 1024 px luồng cuộn ngang trong vùng `Pipeline của task`, trang không tràn ngang |  |  |  |
| UI52-02 | Luồng pipeline ở 480 px | Các nhóm xếp dọc, mỗi nhóm hai thẻ; chân nhóm và nút thẻ Bạn xuống dòng; trang không tràn ngang |  |  |  |
| UI52-03 | Nút thẻ Bạn trong 112 px ở 1024 px (task chờ duyệt plan) | `Duyệt plan` và `Trả về Lead` hiện đủ chữ (có thể hai dòng); `Trả về Lead` có `scrollWidth ≤ clientWidth` |  |  |  |
| UI52-04 | Hàng tiêu đề Pipeline và huy hiệu autopilot ở 480 px | Hàng xuống dòng; chữ huy hiệu hiện đủ, không bị cắt |  |  |  |
| UI52-05 | Độ tương phản sáng / tối: khung hổ phách `ring-2` của nhóm dừng, chữ lý do ở chân nhóm, huy hiệu Autopilot xanh nhấp nháy, nền lựa chọn ở ô Quy trình | Đọc rõ ở cả hai giao diện |  |  |  |
| UI52-06 | Dấu tiếng Việt `Ế Ộ Ữ` ở tiêu đề task, dòng kết quả của thẻ và huy hiệu | Dấu ở dòng trên cùng và dòng cuối không bị cắt, ở 1024 và 480 px |  |  |  |
