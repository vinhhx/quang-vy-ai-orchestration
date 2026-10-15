# Sổ tay nghiệm thu trực tiếp giao thức MVP (Protocol Manual)

Tài liệu hướng dẫn người dùng thực hiện nghiệm thu trực tiếp các ca kiểm thử **P1-01..P1-10** và ghi nhật ký cho **5 task thật** để hoàn thành cổng nghiệm thu Phase 2.

---

## 1. Chuẩn bị

1. **Cài đặt lệnh `brain`**:
   ```bash
   bash brain/install.sh
   ```
   Đảm bảo `~/.local/bin` có trong `$PATH`. Kiểm tra bằng: `brain --help`.

2. **Quét và đăng ký dự án**:
   ```bash
   brain scan --root /duong/dan/thu/muc/chua/cac/du-an
   ```

3. **Cấu hình dự án (`projects_map.json`)**:
   Mở file `~/.config/agent-brain/projects_map.json` và chỉnh sửa tay cho các dự án thật:
   - `name_vi`: Tên tiếng Việt có dấu của dự án (ví dụ: `"Web bán hàng"`).
   - `aliases`: Danh sách tên gọi tắt, biệt danh (ví dụ: `["shop", "ban hang"]`).
   - `profile`: Bắt buộc điền `"work"` hoặc `"personal"`.

4. **Ánh xạ profile tài khoản**:
   - `work`: Sử dụng tài khoản công ty tại `~/.claude` (biến môi trường `CLAUDE_CONFIG_DIR` được bỏ đặt).
   - `personal`: Sử dụng tài khoản cá nhân tại `~/.claude-orca` (`CLAUDE_CONFIG_DIR=~/.claude-orca`).
   > [!WARNING]
   > `~/.claude-orca` cũng là profile của chính phiên Orca/Claude đang điều phối. Chỉ chạy các tác vụ có profile `personal` từ một cửa sổ terminal độc lập khi không có phiên Claude nào khác đang hoạt động trên profile `~/.claude-orca` (rủi ro xoay vòng token E-06 chưa được kiểm chứng, spike S5).

5. **Chọn dự án nháp**:
   Trước khi chạy trên 5 task thật, hãy chuẩn bị một dự án git nháp/tạm thời (throwaway project) để thực hiện các ca thử nghiệm P1-06..P1-09.

---

## 2. Kịch bản kiểm thử trực tiếp (P1-01 .. P1-10)

### P1-01: Gọi bằng tên có dấu "web bán hàng"
- **Lệnh**:
  ```bash
  brain lead "web bán hàng" "Khảo sát tính năng tìm kiếm" --dry-run
  ```
- **Kết quả mong đợi**: Khớp đúng `web-ban-hang` (hoặc id tương ứng trong map). In dòng lệnh dry-run và thoát mã 0.
- **Ghi nhận**: Điền `Đạt` vào [nghiem-thu-p1.md](../protocol-log/nghiem-thu-p1.md).

### P1-02: Gọi bằng alias "shop"
- **Lệnh**:
  ```bash
  brain lead "shop" "Khảo sát tính năng tìm kiếm" --dry-run
  ```
- **Kết quả mong đợi**: Khớp đúng dự án có alias `"shop"`. Thoát mã 0.

### P1-03: Tên mơ hồ khớp 2 dự án
- **Lệnh**:
  ```bash
  brain lead "web" "Khảo sát" --dry-run
  echo "Exit code: $?"
  ```
- **Kết quả mong đợi**: Dừng lại, in danh sách các dự án khớp, không gọi agent. Mã thoát in ra là `2`.

### P1-04: Tên không khớp dự án nào
- **Lệnh**:
  ```bash
  brain lead "du-an-khong-ton-tai-xyz" "Khảo sát" --dry-run
  echo "Exit code: $?"
  ```
- **Kết quả mong đợi**: Dừng lại, in danh sách toàn bộ các dự án hiện có trong map, không gọi agent. Mã thoát in ra là `2`.

### P1-05: Clone dự án mới rồi chạy quét
- **Lệnh**:
  ```bash
  git clone <repo-mau> /duong/dan/scan_roots/du-an-moi
  brain scan --root /duong/dan/scan_roots
  ```
- **Kiểm tra**:
  Xem nội dung `~/.config/agent-brain/projects_map.json`:
  - `du-an-moi` xuất hiện với `profile: ""`.
  - Các trường `name_vi`, `aliases`, `profile` đã sửa tay của các dự án cũ giữ nguyên vẹn.

### P1-06: Lead chạy xong
- **Lệnh**:
  ```bash
  brain lead <du-an-nhap> "Thêm hàm tính tổng"
  ```
- **Lưu ý re-plan**:
  Nếu chạy lại `brain lead` trên cùng một nhánh task, `brain` sẽ tái sinh `.plan.md` từ skeleton mới và ghi đè các sửa đổi tay chưa commit. Để giữ các sửa đổi tay, hãy commit chúng bằng lệnh:
  ```bash
  git -C <root> commit -m "<task>: plan" -- .plan.md
  ```
- **Kiểm tra**:
  1. `git status`: Thư mục làm việc sạch sẽ, không có file code nào bị sửa đổi.
  2. `git show --stat HEAD`: Commit mới nhất là `<task>: plan`, chỉ chứa duy nhất `.plan.md`.

### P1-07: Dev chạy với plan đầy đủ
- **Lệnh**:
  ```bash
  brain dev <du-an-nhap>
  ```
- **Kiểm tra bắt buộc ngay sau khi Dev chạy xong**:
  1. **Kiểm tra guard hook**: Lấy đường dẫn `<run_dir>` từ thông báo log stderr. Kiểm tra file `<run_dir>/guard.jsonl` có tồn tại và có ít nhất một dòng log:
     ```bash
     test -s "<run_dir>/guard.jsonl" && echo "Guard OK" || echo "Guard MISSING"
     ```
     > [!CAUTION]
     > Nếu `<run_dir>/guard.jsonl` bị trống hoặc không tồn tại, guard hook đã không kích hoạt dưới Claude Code CLI thật. Hãy đánh dấu **P1-07 Không đạt** kèm ghi chú này vào [nghiem-thu-p1.md](../protocol-log/nghiem-thu-p1.md) và **DỪNG phiên chạy trực tiếp trước P1-08**.
  2. `git diff --name-only HEAD~1..HEAD`: Chỉ các file được liệt kê trong bảng `Thay đổi` của `.plan.md` bị sửa đổi.
  3. Kiểm tra file nhật ký `protocol-log/<du-an-nhap>--{task}.md`:
     - Dòng `- Số turn của Dev trước khi bắt đầu sửa:` chứa số lượt hợp lệ (≤ 2).
     - Dòng `- Dev có đọc file ngoài plan không: Không`.

### P1-08: Dev chạy với plan thiếu thông tin
- **Chuẩn bị**: Sửa tay file `.plan.md` trên dự án nháp: xóa chi tiết cột `Dòng / hàm` hoặc xóa nội dung `Sửa gì`, rồi commit lại:
  ```bash
  git commit -m "<task>: plan" -- .plan.md
  ```
- **Lệnh**:
  ```bash
  brain dev <du-an-nhap>
  echo "Exit code: $?"
  ```
- **Kết quả mong đợi**: Dev phát hiện thiếu thông tin, tự động ghi mục `Vướng mắc` vào `.plan.md`, không sửa code bừa bãi. Lệnh thoát với mã lỗi `4` (BLOCKED).

### P1-09: QA chạy
- **Lệnh**:
  ```bash
  brain qa <du-an-nhap>
  ```
- **Kiểm tra**:
  1. `git show --stat HEAD`: Có commit mới `<task>: qa`, chỉ thêm file `.qa-report.md`, không sửa file code nào.
  2. Mở `.qa-report.md` kiểm tra mục `## Kết luận` có `Đạt` hoặc `Cần sửa`.

### P1-10: Dự án profile `work`
- **Lệnh**:
  ```bash
  brain lead <du-an-work> "Thử nghiệm cấu hình work" --name "test-work-profile"
  ```
- **Kiểm tra**:
  1. Mở file `stream.jsonl` trong `<run_dir>` để lấy giá trị `session_id`.
  2. Kiểm tra file transcript thực sự nằm trong profile work:
     ```bash
     ls ~/.claude/projects/*/<session_id>.jsonl
     ```
  3. Đảm bảo file session đó KHÔNG tồn tại trong profile personal:
     ```bash
     test ! -e ~/.claude-orca/projects/*/<session_id>.jsonl
     ```

---

## 3. Thực hiện 5 task thật

Sau khi P1-01..P1-10 đạt yêu cầu, tiến hành chạy trên 5 task thật của các dự án thực tế:

1. **Chu trình thực hiện mỗi task**:
   - Bước 1: `brain lead "<du-an>" "<yeu-cau>"` → Review `.plan.md`.
   - Bước 2: `brain dev "<du-an>"` → Dev thực thi kế hoạch.
   - Bước 3: `brain qa "<du-an>"` → QA đánh giá chất lượng.
2. **Điền nhật ký thủ công**:
   Mở file nhật ký đã được tự động sinh tại `protocol-log/<du-an>--{task}.md` và điền hai dòng:
   - `- QA phát hiện được gì: <chi tiết phát hiện của QA>`
   - `- Phải làm lại không, vì sao: <Có/Không, lý do cụ thể>`
3. **Kiểm tra cổng hoàn thành Phase 2**:
   ```bash
   python3 -m brain.tasklog check protocol-log --min 5
   ```
   Lệnh phải in ra `5/5 nhật ký đủ 6 dòng` và thoát mã `0`.

---

## 4. Xử lý sự cố (Troubleshooting)

| Hiện tượng | Nguyên nhân có thể | Cách xử lý |
|---|---|---|
| Lỗi `--verbose` khi chạy agent | Thiếu `--verbose` khi dùng `--output-format stream-json` | Xác nhận giả định A-01: bắt buộc giữ `--verbose`. |
| `guard.jsonl` trống hoặc thiếu | Claude CLI không gọi PreToolUse hook cấu hình qua `--settings` | Kiểm tra giả định A-05; kiểm tra cú pháp matcher hook trong `guard.py`. |
| `permission_denials` xuất hiện trong bảng nhật ký | Công cụ cần cấp quyền nhưng không khớp cú pháp allowlist | Kiểm tra giả định A-03; xem lại cú pháp `--allowedTools`. |
| Cảnh báo cờ lạ `--strict-mcp-config` | Phiên bản Claude CLI không hỗ trợ cờ này | Sửa `common_args` trong `brain/config.json` để bỏ cờ (A-07). |
| Quá thời gian chạy (timeout) hoặc ngắt Ctrl+C | Task quá lớn hoặc tiến trình con bị treo | Tăng `timeout_seconds` trong `config.json` (A-09). |
