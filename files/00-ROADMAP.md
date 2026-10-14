# Rust Agent Brain — Roadmap tổng

> Mục tiêu: một bộ điều phối (orchestrator) nhẹ, chạy 1 file binary, để giao việc cho nhiều AI agent (Claude, DeepSeek, Antigravity…) trên mọi dự án: code, video, khách hàng. Thay thế Paperclip.

## 1. Nguyên tắc xuyên suốt

1. **Giao thức trước, giao diện sau.** Phần mang lại giá trị là cách các agent phối hợp (Lead → `.plan.md` → Dev → QA). Dashboard chỉ là lớp hiển thị.
2. **Mỗi giai đoạn phải dùng được ngay.** Không có giai đoạn nào chỉ là "chuẩn bị". Hết mỗi phase là bạn làm việc thật bằng nó.
3. **Đo trước khi tối ưu.** Không tối ưu token (session pool, golden session) khi chưa có số liệu token thật từ Phase 2.
4. **Kiểm chứng giả định sớm.** Mọi cờ CLI, endpoint, khả năng headless phải được thử trong Phase 0 trước khi thiết kế dựa vào chúng.
5. **Bạn là tester, nên mỗi phase có tiêu chí nghiệm thu dạng test case.** Agent viết code, bạn nghiệm thu.

## 2. Các giai đoạn

| Phase | Tên | Giải quyết vấn đề (theo bảng gốc) | Đầu ra chính | Cổng qua phase |
|---|---|---|---|---|
| 0 | Kiểm chứng giả định | Nền cho tất cả | `spike-results.md` | Mọi giả định có kết quả Đạt/Không đạt |
| 1 | Giao thức MVP (script) | #5, #6, #11 | `projects_map.json`, 3 prompt Lead/Dev/QA, script chạy | Hoàn thành 5 task thật bằng giao thức |
| 2 | Lõi Rust (CLI, chưa có UI) | #1, #3 (phần bắt log), #6, #9 | binary `agent-brain` dạng CLI + SQLite | Kill sạch cây tiến trình; log token đầy đủ |
| 3 | Tối ưu session & token | #7, #8 | Chiến lược session có số liệu chứng minh | Giảm token/task đo được so với Phase 2 |
| 4 | Dashboard nhúng | #2, #3 | Web UI + SSE trong cùng binary | Xem log realtime, kill từ UI |
| 5 | Đa đội hình & nhánh Media | #4, #10 | Routing engine linh hoạt, pipeline video tách riêng | Chạy 1 task trộn engine end-to-end |

## 3. Thứ tự phụ thuộc

```
Phase 0 ──► Phase 1 ──► Phase 2 ──┬──► Phase 3
                                  ├──► Phase 4
                                  └──► Phase 5
```

Sau Phase 2, ba phase còn lại độc lập nhau. Chọn thứ tự theo nỗi đau lớn nhất lúc đó:
- Hết quota thường xuyên → làm Phase 5 (routing) trước.
- Token/task cao → Phase 3 trước.
- Khó theo dõi agent → Phase 4 trước.

## 4. Ước lượng thời gian (tham khảo)

Giả định: làm ngoài giờ, agent viết code, bạn review và nghiệm thu.

| Phase | Ước lượng | Ghi chú |
|---|---|---|
| 0 | 2–4 buổi | Chủ yếu chạy lệnh và ghi kết quả |
| 1 | 1–2 tuần | Phần lớn thời gian là dùng thử và chỉnh prompt |
| 2 | 2–4 tuần | Phase rủi ro nhất vì là Rust async |
| 3 | 1–2 tuần | Phụ thuộc số liệu thu được |
| 4 | 2–3 tuần | |
| 5 | 2–4 tuần | Phụ thuộc kết quả Phase 0 về Antigravity |

## 5. Chỉ số đo (KPI) xuyên suốt

| Chỉ số | Cách đo | Mục tiêu |
|---|---|---|
| Token trung bình / task | SQLite (từ Phase 2) | Giảm dần qua các phase |
| Số turn trước khi agent bắt đầu sửa code | Đếm trong log stream-json | ≤ 2 |
| Task chạy nhầm repo | Ghi tay / log | 0 |
| Tiến trình còn sót sau Kill | `ps` sau khi kill | 0 |
| Task phải làm lại do Dev hiểu sai plan | Ghi tay | Giảm dần |

## 6. Danh sách tài liệu

- `00-ROADMAP.md` — tài liệu này
- `01-phase0-kiem-chung.md`
- `02-phase1-giao-thuc-mvp.md`
- `03-phase2-loi-rust.md`
- `04-phase3-toi-uu-session.md`
- `05-phase4-dashboard.md`
- `06-phase5-da-doi-hinh-media.md`

## 7. Câu hỏi còn mở

- Hai tài khoản Claude đều do công ty cấp: có được phép dùng một tài khoản cho dự án cá nhân và dựng video không? Cần xác nhận trước Phase 2 (profile Work/Personal).
- Antigravity có chế độ chạy headless và xuất log có cấu trúc không? (Phase 0 trả lời)
- Chọn dashboard trước (Phase 4) hay routing trước (Phase 5)? Quyết định sau Phase 2.
