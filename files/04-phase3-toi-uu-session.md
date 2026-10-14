# Phase 3 — Tối ưu session & token

## 1. Mục tiêu

Giảm token mỗi task dựa trên số liệu thật từ SQLite (Phase 2), không dựa trên ước đoán.

Giải quyết: #7 (context phình to), #8 (tốn token đọc lại `.gsd`).

## 2. Phân tích

### 2.1 Điều kiện bắt đầu
Cần ít nhất 2 tuần dữ liệu `runs` từ Phase 2 để biết:
- Token trung bình mỗi vai trò (Lead / Dev / QA).
- Bao nhiêu token rơi vào việc đọc `.gsd` và khảo sát ban đầu.
- Các task có chạy sát nhau (trong vài phút) hay rải rác.

### 2.2 Giới hạn của prompt cache
- Cache sống khoảng 5 phút. Session gốc nguội thì fork không được giảm giá, vẫn phải xử lý lại toàn bộ context.
- Fork giúp tiết kiệm **số turn** (agent không phải mò lại), nhưng **không chắc tiết kiệm token** nếu cache đã hết hạn.
- Với gói subscription, cache ảnh hưởng tới quota thế nào cần đo thực tế (kết quả spike S3).

### 2.3 So sánh phương án nạp ngữ cảnh `.gsd`

| Phương án | Độ phức tạp | Tiết kiệm turn | Tiết kiệm token | Ghi chú |
|---|---|---|---|---|
| A. Tóm tắt `.gsd` vào `CLAUDE.md` / `--append-system-prompt` | Thấp | Cao | Trung bình | Không quản lý session, dùng được với mọi engine hỗ trợ system prompt |
| B. Golden session + fork | Cao | Cao | Cao chỉ khi cache còn ấm | Cần theo dõi mtime/hash `.gsd`, tạo lại session gốc |
| C. Kết hợp: A mặc định, B khi chạy nhiều task liên tiếp | Trung bình | Cao | Cao | Chỉ nên làm nếu số liệu cho thấy có nhiều task liên tiếp |

**Đề xuất:** làm A trước, đo lại. Chỉ làm B nếu số liệu cho thấy còn đáng kể token bị lãng phí và các task thường chạy sát nhau.

### 2.4 Kiểm soát độ dài session
- Theo dõi tổng token context của session sau mỗi run (lấy từ usage).
- Đặt ngưỡng (ban đầu 45.000, điều chỉnh theo số liệu).
- Vượt ngưỡng → tạo `.handover.md` (tóm tắt: đã làm gì, đang dở gì, quyết định đã chốt, file liên quan) rồi mở session mới nạp handover.

## 3. Thiết kế

### 3.1 Mẫu `.gsd-summary.md` (cho phương án A)
```markdown
# <Dự án> — tóm tắt cho agent
## Công nghệ
## Cấu trúc thư mục chính
## Quy ước code
## Lệnh build / test
## Những chỗ nhạy cảm, không tự ý sửa
```
Tự tạo lại khi hash `.gsd` thay đổi.

### 3.2 Mẫu `.handover.md`
```markdown
# Handover: <task>
## Đã hoàn thành
## Đang dở
## Quyết định đã chốt
## File liên quan
## Bước tiếp theo
```

### 3.3 Bổ sung SQLite
```
sessions(id, project_id, task_id, kind, parent_session_id,
         context_tokens, gsd_hash, created_at, last_used_at)
-- kind: base | task | fork
```

## 4. Công việc

- [ ] Viết truy vấn báo cáo: token trung bình theo vai trò, theo dự án
- [ ] Đo token của 3 turn đầu mỗi run (chi phí "mò đường")
- [ ] Làm phương án A: sinh `.gsd-summary.md` theo hash
- [ ] Đo lại 1 tuần, so sánh
- [ ] Thêm theo dõi độ dài session + `.handover.md`
- [ ] Quyết định có làm phương án B không (ghi lý do)

## 5. Tiêu chí nghiệm thu

| ID | Kịch bản | Kết quả mong đợi |
|---|---|---|
| P3-01 | Báo cáo token theo vai trò | Có số liệu trước/sau tối ưu |
| P3-02 | Run Dev với `.gsd-summary.md` | Bắt đầu sửa trong ≤ 2 turn |
| P3-03 | Sửa `.gsd` | Summary tự tạo lại ở lần chạy sau |
| P3-04 | Session vượt ngưỡng | Có `.handover.md`, session mới tiếp tục đúng việc |
| P3-05 | Session mới từ handover | Không hỏi lại những quyết định đã chốt |
| P3-06 | Token/task trung bình | Giảm đo được so với Phase 2 |

## 6. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Summary thiếu thông tin quan trọng, agent làm sai | QA kiểm tra; bổ sung mục vào mẫu summary khi phát hiện |
| Handover làm mất ngữ cảnh | Giữ session cũ để tra cứu, không xóa |
| Tối ưu nhưng không đo lại | Không đóng phase khi chưa có P3-06 |
