# Phase 1 — Giao thức MVP (chưa có Rust)

## 1. Mục tiêu

Chạy được quy trình Lead → Dev → QA trên dự án thật chỉ bằng script và file cấu hình. Chứng minh giao thức hiệu quả trước khi đầu tư xây app.

Giải quyết: #5 (định tuyến dự án tiếng Việt), #6 (tách profile, ở mức biến môi trường), #11 (giao thức thắt cổ chai thông tin).

## 2. Phạm vi

**Trong phạm vi:** `projects_map.json`, script quét thư mục, 3 prompt vai trò, file `.plan.md`, script chạy từng vai trò.

**Ngoài phạm vi:** dashboard, SQLite, tối ưu session, chạy song song nhiều task.

## 3. Phân tích

### 3.1 Vì sao làm giao thức trước
Giao thức là phần không phụ thuộc công cụ: nó chạy được với Claude, DeepSeek hay bất kỳ engine nào. Nếu giao thức không hiệu quả (Dev vẫn phải quét lại dự án, plan thiếu tọa độ), thì app Rust phía sau cũng không cứu được.

### 3.2 Định tuyến dự án
Vấn đề: tên thư mục không dấu, lệnh nói bằng tiếng Việt có dấu, dự án mới liên tục thêm vào.

Giải pháp:
- Script quét thư mục gốc, thêm dự án mới vào `projects_map.json` (không ghi đè alias đã sửa tay).
- So khớp tên: bỏ dấu cả hai phía, so với `name_vi` và `aliases`.
- **Luật Fallback Ask:** khớp 0 hoặc nhiều hơn 1 dự án → dừng, liệt kê danh sách, hỏi lại. Không bao giờ đoán.

### 3.3 Giao thức 3 vai trò

| Vai trò | Được làm | Cấm | Đầu vào | Đầu ra |
|---|---|---|---|---|
| Lead | Đọc code, khảo sát | Sửa file code | Yêu cầu task + `.gsd` | `.plan.md` |
| Dev | Sửa đúng vị trí trong plan | Quét lại dự án, sửa ngoài plan | `.plan.md` | Commit trên nhánh task |
| QA | Đọc diff, chỉ ra rủi ro | Sửa code | `git diff` + `.plan.md` | `.qa-report.md` |

Nếu Dev thấy plan sai hoặc thiếu: ghi vào mục "Vướng mắc" trong `.plan.md` rồi dừng, không tự sửa ngoài phạm vi.

## 4. Thiết kế

### 4.1 `projects_map.json`
```json
{
  "root": "~/projects",
  "projects": [
    {
      "id": "web-ban-hang",
      "path": "~/projects/web-ban-hang",
      "name_vi": "Web bán hàng",
      "aliases": ["shop", "bán hàng", "web shop"],
      "type": "code",
      "repo": "p.zamba",
      "profile": "work"
    }
  ]
}
```
`type`: `code` hoặc `media` (chuẩn bị cho Phase 5). `profile`: `work` hoặc `personal`.

### 4.2 Mẫu `.plan.md`
```markdown
# Plan: <tên task>
## Mục tiêu
## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
## Không được đụng tới
## Cách kiểm tra
## Vướng mắc (Dev ghi vào đây nếu có)
```

### 4.3 Bản nháp prompt

**Lead**
```
Bạn là Lead. Nhiệm vụ: khảo sát dự án và viết kế hoạch sửa, KHÔNG sửa code.
1. Đọc .gsd và các file liên quan đến yêu cầu.
2. Ghi .plan.md theo đúng mẫu: mỗi thay đổi có file, dòng/hàm, nội dung sửa, lý do.
3. Liệt kê rõ những phần không được đụng tới.
4. Ghi cách kiểm tra sau khi sửa.
Dừng sau khi ghi xong .plan.md.
```

**Dev**
```
Bạn là Dev. Chỉ đọc .plan.md và các file được nêu trong đó.
- Không tìm kiếm hay đọc file nào khác ngoài plan.
- Sửa đúng các mục trong bảng Thay đổi, không thêm cải tiến ngoài plan.
- Nếu plan sai hoặc thiếu thông tin: ghi vào mục Vướng mắc và dừng.
- Commit với message: "<task>: <mô tả ngắn>".
```

**QA**
```
Bạn là QA. Đầu vào: git diff và .plan.md. Không sửa code.
Ghi .qa-report.md gồm:
- Thay đổi có khớp plan không (từng mục).
- Sửa ngoài plan (nếu có).
- Rủi ro: lỗi logic, trường hợp biên, ảnh hưởng tới chỗ khác.
- Kết luận: Đạt / Cần sửa.
```

### 4.4 Script chạy (ý tưởng)
```
brain lead  "<dự án>" "<yêu cầu>"   → định tuyến → tạo nhánh → chạy Lead
brain dev   "<dự án>"               → chạy Dev đọc .plan.md
brain qa    "<dự án>"               → chạy QA trên git diff
```
Ở phase này có thể là shell script. Profile chọn qua `CLAUDE_CONFIG_DIR` theo trường `profile` của dự án.

## 5. Công việc

- [ ] Viết script quét thư mục → `projects_map.json`
- [ ] Sửa tay `name_vi` và `aliases` cho các dự án hiện có
- [ ] Viết hàm so khớp tên không dấu + Fallback Ask
- [ ] Hoàn thiện 3 prompt (lưu thành file riêng, có thể dùng `--append-system-prompt`)
- [ ] Viết script `brain lead / dev / qa`
- [ ] Chạy 5 task thật, ghi nhật ký sau mỗi task

## 6. Tiêu chí nghiệm thu (test case)

| ID | Kịch bản | Kết quả mong đợi |
|---|---|---|
| P1-01 | Gọi bằng tên có dấu "web bán hàng" | Khớp đúng `web-ban-hang` |
| P1-02 | Gọi bằng alias "shop" | Khớp đúng |
| P1-03 | Tên mơ hồ khớp 2 dự án | Dừng, liệt kê 2 dự án, không chạy agent |
| P1-04 | Tên không khớp dự án nào | Dừng, liệt kê toàn bộ, không chạy agent |
| P1-05 | Clone dự án mới rồi chạy quét | Dự án mới xuất hiện, alias cũ không bị mất |
| P1-06 | Lead chạy xong | Không có file code nào bị sửa (`git status` sạch ngoài `.plan.md`) |
| P1-07 | Dev chạy với plan đầy đủ | Chỉ file trong plan bị sửa |
| P1-08 | Dev chạy với plan thiếu thông tin | Ghi Vướng mắc, không sửa bừa |
| P1-09 | QA chạy | Có `.qa-report.md`, không sửa code |
| P1-10 | Dự án profile `work` | Chạy đúng `CLAUDE_CONFIG_DIR` của work |

## 7. Nhật ký sau mỗi task (để làm số liệu cho Phase 3)

```markdown
- Task:
- Dự án:
- Số turn của Dev trước khi bắt đầu sửa:
- Dev có đọc file ngoài plan không:
- QA phát hiện được gì:
- Phải làm lại không, vì sao:
```

## 8. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Agent không tuân thủ lệnh cấm (Dev vẫn quét dự án) | Giới hạn công cụ bằng `--allowedTools` / `--disallowedTools`, không chỉ dựa vào prompt |
| Plan quá chung chung | Thêm ví dụ plan tốt vào prompt Lead |
| Alias trùng giữa các dự án | Script quét cảnh báo alias trùng |
