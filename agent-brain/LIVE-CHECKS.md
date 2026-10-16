# Sổ tay nghiệm thu trực tiếp lõi Rust (Live Checks)

Tài liệu này cung cấp hướng dẫn từng bước và các câu lệnh chính xác để người dùng thực hiện nghiệm thu trực tiếp các ca kiểm thử tiêu tốn token hoặc tương tác terminal thật cho lõi Rust `agent-brain` (Phase 3 / kế hoạch 03-13).

Kết quả nghiệm thu sẽ được người dùng điền vào bảng tổng hợp tại [`protocol-log/nghiem-thu-p2.md`](../protocol-log/nghiem-thu-p2.md).

> [!CAUTION]
> **Quy tắc an toàn tiến trình**: Tuyệt đối không dùng các lệnh ngắt tiến trình hàng loạt theo tên. Việc dừng tiến trình chỉ được thực hiện thông qua lệnh `agent-brain kill <task_id>`, tín hiệu Ctrl+C từ bàn phím hoặc đóng cửa sổ terminal. Lệnh `ps` chỉ được dùng ở chế độ đọc với danh sách PID cụ thể đã ghi nhận.

---

## 1. Chuẩn bị môi trường

1. **Dự án nháp (Throwaway repo)**:
   - Sử dụng thư mục `/mnt/data/IDE/du-an-nhap` trên nhánh `main` sạch sẽ.
   - Dự án đã được đăng ký trong `~/.config/agent-brain/projects_map.json` với `name_vi: "dự án nháp"` và `profile: "work"`.
   - File `calc.py` cơ bản đã được commit lần đầu trên `main`.

2. **Quy trình dọn nhánh sau mỗi lần kill**:
   Sau mỗi lần ngắt hoặc kill một tác vụ thử nghiệm, thực hiện dọn dẹp nhánh task vừa tạo để đưa repo về trạng thái sạch:
   ```bash
   git -C /mnt/data/IDE/du-an-nhap checkout -f main
   git -C /mnt/data/IDE/du-an-nhap branch -D task/{name}
   ```
   (Chỉ áp dụng với đúng tên nhánh `{name}` do ca kiểm thử đó vừa tạo ra).

---

## 2. Kịch bản nghiệm thu chi tiết

### P2-01: Chạy `run lead` trên dự án thật
- **Mục tiêu**: Kiểm tra hiển thị log stream từng bước trên terminal và ghi nhận đầy đủ khi kết thúc.
- **Tiền điều kiện**: Dự án `/mnt/data/IDE/du-an-nhap` đang ở nhánh `main` sạch sẽ.
- **Lệnh thực hiện**:
  ```bash
  agent-brain run lead "dự án nháp" "thêm hàm tính hiệu hai số vào calc.py"
  ```
- **Quan sát & Kết quả mong đợi**:
  1. Terminal hiển thị từng bước sự kiện trực tiếp từ Claude (init, turn, tool calls, assistant text).
  2. Dòng cuối cùng in tóm tắt: `Kết thúc run ...` với số lượt (turns), token và trạng thái hoàn thành.
  3. Kiểm tra trạng thái dự án:
     ```bash
     agent-brain status "dự án nháp"
     ```
     Lệnh chạy tức thì (0 token), hiển thị nhánh `task/...` vừa tạo và commit plan.
- **Ghi nhận**: Điền `Đạt` vào dòng `P2-01` trong `protocol-log/nghiem-thu-p2.md`.
- **Dọn dẹp**:
  ```bash
  git -C /mnt/data/IDE/du-an-nhap checkout -f main
  ```

---

### P2-02: Kill khi agent đang chạy `sleep 300`
- **Mục tiêu**: Kiểm tra khả năng ngắt triệt để cây tiến trình qua lệnh `agent-brain kill`, không để sót tiến trình con hay cháu.
- **Chuẩn bị cấu hình wrapper cho phép sleep**:
  Claude Code ở chế độ bình thường không có quyền chạy `sleep`. Sử dụng file wrapper `scripts/live/claude-allow-sleep.sh` để thêm quyền `Bash(sleep *)` cho đợt kiểm tra này:
  ```bash
  python3 -c "import json, os; c = json.load(open('agent-brain/assets/config.default.json')); c['claude_bin'] = os.path.abspath('scripts/live/claude-allow-sleep.sh'); json.dump(c, open('/tmp/ab-live.json', 'w'), indent=2)"
  ```
- **Lệnh thực hiện (Terminal 1)**:
  ```bash
  AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này và chờ nó xong: sleep 307" --name p202-kill
  ```
- **Thực hiện kill (Terminal 2)**:
  1. Khi thấy dòng log `tool Bash sleep 307` xuất hiện trên Terminal 1, mở Terminal 2 và quan sát cây tiến trình:
     ```bash
     ps -eo pid,ppid,pgid,sid,args --forest
     ```
     Ghi lại 3 PID: tiến trình Claude (nằm dưới agent-brain), tiến trình con bash, và tiến trình cháu `sleep 307`.
  2. Đọc mã task `N` từ dòng log khởi tạo `task N, run M` ở Terminal 1.
  3. Chạy lệnh kill:
     ```bash
     agent-brain kill N
     ```
  4. Chờ 5 giây (`sleep 5`), sau đó kiểm tra lại các PID đã ghi nhận:
     ```bash
     ps -p <claude>,<bash>,<sleep> -o pid,args
     ```
- **Kết quả mong đợi**:
  - Lệnh `ps` chỉ in dòng tiêu đề `PID ARGS`, cả 3 tiến trình đều đã biến mất hoàn toàn.
  - Kiểm tra trạng thái:
     ```bash
     agent-brain status "dự án nháp"
     ```
     Hiển thị trạng thái run là `killed`.
- **Ghi nhận**: Điền `Đạt` vào dòng `P2-02` trong `protocol-log/nghiem-thu-p2.md`.
- **Dọn dẹp**:
  ```bash
  git -C /mnt/data/IDE/du-an-nhap checkout -f main
  git -C /mnt/data/IDE/du-an-nhap branch -D task/p202-kill
  ```

---

### P2-03: Đóng terminal (Ctrl+C) giữa chừng
- **Mục tiêu**: Kiểm tra không còn tiến trình con sót lại khi nhận các tín hiệu ngắt từ terminal.
- **Tiền điều kiện**: Sử dụng cấu hình `/tmp/ab-live.json` như P2-02.
- **Thực hiện 3 kịch bản phụ**:
  1. **Kịch bản A (Ctrl+C một lần)**:
     - Chạy:
       ```bash
       AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này và chờ nó xong: sleep 307" --name p203-a
       ```
     - Khi thấy `tool Bash sleep 307`, ghi nhận PID của claude và sleep qua terminal khác.
     - Nhấn `Ctrl+C` một lần trên terminal chạy.
     - Chờ trong vòng 5 giây, kiểm tra `ps -p <claude>,<sleep> -o pid,args`: không còn tiến trình nào.
     - `agent-brain status "dự án nháp"` hiển thị run là `killed`.
     - Dọn dẹp: `git -C /mnt/data/IDE/du-an-nhap checkout -f main && git -C /mnt/data/IDE/du-an-nhap branch -D task/p203-a`.
  2. **Kịch bản B (Ctrl+C hai lần nhanh)**:
     - Chạy với `--name p203-b`.
     - Nhấn `Ctrl+C` hai lần liên tiếp thật nhanh (< 1 giây).
     - Kiểm tra các PID tương tự: toàn bộ nhóm tiến trình lập tức bị ngắt.
     - Dọn dẹp: `git -C /mnt/data/IDE/du-an-nhap checkout -f main && git -C /mnt/data/IDE/du-an-nhap branch -D task/p203-b`.
  3. **Kịch bản C (Đóng cửa sổ terminal)**:
     - Mở một cửa sổ terminal mới, chạy với `--name p203-c`.
     - Khi `sleep 307` đang chạy, đóng hẳn cửa sổ terminal đó (gửi SIGHUP).
     - Tại terminal còn lại, kiểm tra các PID: toàn bộ tiến trình biến mất trong vòng 5 giây.
     - Dọn dẹp: `git -C /mnt/data/IDE/du-an-nhap checkout -f main && git -C /mnt/data/IDE/du-an-nhap branch -D task/p203-c`.
- **Ghi nhận**: Điền `Đạt` vào dòng `P2-03` trong `protocol-log/nghiem-thu-p2.md`.

---

### P2-07: Chạy lệnh `inspect`
- **Mục tiêu**: Kiểm tra lệnh `inspect` chỉ tốn đúng 1 turn, đọc diff chưa commit và không để lại session file lưu rác.
- **Tiền điều kiện**: Dự án `/mnt/data/IDE/du-an-nhap` ở nhánh `main`.
- **Lệnh thực hiện**:
  1. Tạo sửa đổi chưa commit trên file `calc.py`:
     ```bash
     echo "# test uncommitted diff" >> /mnt/data/IDE/du-an-nhap/calc.py
     ```
  2. Tạo mốc thời gian kiểm tra:
     ```bash
     touch /tmp/ab-p207-stamp
     ```
  3. Chạy lệnh inspect:
     ```bash
     agent-brain inspect "dự án nháp"
     ```
  4. Đếm số transcript file mới sinh ra:
     ```bash
     find ~/.claude/projects/-mnt-data-IDE-du-an-nhap -name '*.jsonl' -newer /tmp/ab-p207-stamp | wc -l
     ```
  5. Xem nhật ký thống kê:
     ```bash
     agent-brain stats --json
     ```
- **Kết quả mong đợi**:
  - Lệnh `inspect` thoát mã 0, in tóm tắt đánh giá ngắn gọn về diff.
  - Số file jsonl mới đếm được bằng 0 (phiên làm việc tạm đã được dọn sạch).
  - Bản ghi `stats` xuất hiện một dòng có `role: "inspect"` với `num_turns: 1`.
  - *Lưu ý giả định A1*: Nếu lệnh thoát mã 1 và báo lỗi liên quan tới `--max-turns`, ghi nhận giả định A1 bị bác bỏ.
- **Ghi nhận**: Điền `Đạt` vào dòng `P2-07` trong `protocol-log/nghiem-thu-p2.md`.
- **Dọn dẹp**:
  ```bash
  git -C /mnt/data/IDE/du-an-nhap checkout -- calc.py
  rm -f /tmp/ab-p207-stamp
  ```

---

### P2-09: Phân phối binary độc lập (DIST-01)
- **Mục tiêu**: Đảm bảo binary tĩnh chạy được ở thư mục khác hoặc máy khác mà không cần cài đặt Rust hay thư viện chia sẻ.
- **Lệnh thực hiện**:
  ```bash
  bash scripts/build-release.sh
  ```
  Copy binary sang thư mục khác (hoặc máy Linux x86_64 khác):
  ```bash
  mkdir -p /tmp/test-dist
  cp agent-brain/target/x86_64-unknown-linux-musl/release/agent-brain /tmp/test-dist/
  /tmp/test-dist/agent-brain --version
  /tmp/test-dist/agent-brain stats
  ```
- **Kết quả mong đợi**: Cả hai lệnh chạy thành công, thoát mã 0, không báo thiếu thư viện liên kết `.so`.
- **Ghi nhận**: Điền `Đạt` vào dòng `P2-09` trong `protocol-log/nghiem-thu-p2.md`.

---

### D-02: Kiểm chứng tính tương đương trên một task thật & Gỡ Python symlink
- **Mục tiêu**: Chạy hoàn chỉnh một luồng Lead → Dev → QA trên một repository thực tế (không phải repo công cụ), sau đó chuyển giao hoàn toàn quyền điều phối cho binary Rust.
- **Các bước thực hiện**:
  1. Chọn một repo dự án thật độc lập (profile `work`).
  2. Khởi tạo task với Lead:
     ```bash
     agent-brain run lead "<tên dự án>" "<yêu cầu nghiệp vụ>"
     ```
  3. Kiểm tra và chỉnh sửa file `.plan.md` trên nhánh task mới tạo nếu cần.
  4. Thực thi Dev:
     ```bash
     agent-brain run dev "<tên dự án>"
     ```
  5. Tự chạy kiểm thử của dự án theo hướng dẫn in ra trên màn hình.
  6. Chạy QA kiểm tra chất lượng:
     ```bash
     agent-brain run qa "<tên dự án>"
     ```
  7. Xem báo cáo thống kê:
     ```bash
     agent-brain stats --profile work
     ```
  8. Cài đặt binary vào PATH:
     ```bash
     install -m 755 agent-brain/target/x86_64-unknown-linux-musl/release/agent-brain ~/.local/bin/agent-brain
     ```
  9. Gỡ bỏ symlink Python `brain`:
     ```bash
     bash brain/install.sh --uninstall
     ```
  10. Kiểm tra xác nhận: `command -v brain` không còn trả về đường dẫn nào.
- **Ghi nhận**: Điền `Đạt` vào dòng `D-02` trong `protocol-log/nghiem-thu-p2.md`.

---

### E-06: Xác minh cách ly tài khoản cá nhân (Personal Profile)
- **Mục tiêu**: Đảm bảo phiên chạy với profile `personal` chỉ ghi transcript vào `~/.claude-orca` mà không đụng chạm tới `~/.claude`.
- **Tiền điều kiện**: Chỉ thực hiện khi không có bất kỳ phiên Claude nào khác đang mở trên `~/.claude-orca`.
- **Lệnh thực hiện**:
  1. Chọn một dự án có `profile: "personal"` trong `projects_map.json`.
  2. Chạy Lead:
     ```bash
     agent-brain run lead "<dự án cá nhân>" "<yêu cầu thử nghiệm>"
     ```
  3. Đọc mã `session <id>` từ dòng log `init · session <id>`.
  4. Kiểm tra đường dẫn lưu file transcript:
     ```bash
     find ~/.claude-orca/projects ~/.claude/projects -name '<id>.jsonl'
     ```
- **Kết quả mong đợi**: File chỉ xuất hiện dưới đường dẫn `~/.claude-orca/projects/...`, tuyệt đối không có file nào dưới `~/.claude/projects/...`.
- **Ghi nhận**: Điền `Đạt` vào dòng `E-06` trong `protocol-log/nghiem-thu-p2.md`.
