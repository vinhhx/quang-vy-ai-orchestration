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
   - File `src/calc.py` cơ bản đã được commit lần đầu trên `main`.

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

### P2-02: Kill khi agent đang chạy lệnh dài (python sleep 307)
- **Mục tiêu**: Kiểm tra khả năng ngắt triệt để cây tiến trình qua lệnh `agent-brain kill`, không để sót tiến trình con hay cháu.
- **Lưu ý**: Claude Code 2.1.301 chặn lệnh `sleep N` ở foreground ("Blocked: standalone sleep"), do đó các ca kiểm tra dùng `python3 -c 'import time; time.sleep(307)'`, và wrapper cho phép `Bash(python3 *)` chỉ riêng cho các ca kiểm tra này.
- **Chuẩn bị cấu hình wrapper cho phép sleep**:
  Sử dụng file wrapper `scripts/live/claude-allow-sleep.sh` để thêm quyền `Bash(python3 *)` cho đợt kiểm tra này:
  ```bash
  python3 -c "import json, os; c = json.load(open('agent-brain/assets/config.default.json')); c['claude_bin'] = os.path.abspath('scripts/live/claude-allow-sleep.sh'); json.dump(c, open('/tmp/ab-live.json', 'w'), indent=2)"
  ```
- **Lệnh thực hiện (Terminal 1)**:
  ```bash
  AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'" --name p202-kill
  ```
- **Thực hiện kill (Terminal 2)**:
  1. Khi thấy dòng log `tool Bash python3 -c 'import time; time.sleep(307)'` xuất hiện trên Terminal 1, mở Terminal 2 và quan sát cây tiến trình cùng PID chính xác:
     ```bash
     ps -eo pid,ppid,pgid,sid,args --forest | grep -A3 '[a]gent-brain run lead'
     pgrep -f '^python3 -c import time; time.sleep\(307\)'
     ```
     Ghi lại 3 PID: tiến trình Claude (nằm dưới `agent-brain`), tiến trình con bash, và tiến trình cháu `python3`.
     *Lưu ý*: Lệnh tìm `pgrep -f time.sleep` thông thường sẽ khớp cả `agent-brain` và `claude` do câu lệnh request nằm trong danh sách đối số (`argv`) của chúng. Hơn nữa, tiến trình con bash chạy trong PGID và SID riêng, nên việc kiểm tra phải bao quát toàn bộ từng PID đã ghi nhận (`<claude>,<bash>,<python>`), chứ không chỉ riêng process group.
  2. Đọc mã task `N` từ dòng log khởi tạo `task N, run M` ở Terminal 1.
  3. Chạy lệnh kill:
     ```bash
     agent-brain kill N
     ```
  4. Chờ 5 giây (`sleep 5`), sau đó kiểm tra lại các PID đã ghi nhận:
     ```bash
     ps -p <claude>,<bash>,<python> -o pid,args
     pgrep -af '^python3 -c import time; time.sleep\(307\)' || echo sạch
     ```
- **Kết quả mong đợi**:
  - Lệnh `ps` chỉ in dòng tiêu đề `PID ARGS`, cả 3 tiến trình đều đã biến mất hoàn toàn.
  - Lệnh `pgrep` in `sạch`.
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
  rm -f /mnt/data/IDE/du-an-nhap/.plan.md
  ```

---

### P2-03: Đóng terminal (Ctrl+C) giữa chừng
- **Mục tiêu**: Kiểm tra không còn tiến trình con sót lại khi nhận các tín hiệu ngắt từ terminal.
- **Tiền điều kiện**: Sử dụng cấu hình `/tmp/ab-live.json` như P2-02.
- **Thực hiện 3 kịch bản phụ**:
  1. **Kịch bản A (Ctrl+C một lần)**:
     - Chạy:
       ```bash
       AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'" --name p203-a
       ```
     - Khi thấy dòng log `tool Bash python3 -c 'import time; time.sleep(307)'`, ghi nhận các PID `<claude>,<bash>,<python>` qua terminal khác (`ps -eo pid,ppid,pgid,sid,args --forest | grep -A3 '[a]gent-brain run lead'`).
     - Nhấn `Ctrl+C` một lần trên terminal chạy.
     - Chờ trong vòng 5 giây (`sleep 5`), kiểm tra:
       ```bash
       ps -p <claude>,<bash>,<python> -o pid,args
       pgrep -af '^python3 -c import time; time.sleep\(307\)' || echo sạch
       ```
       Không còn tiến trình nào sót lại (lệnh `ps` chỉ in tiêu đề `PID ARGS`, `pgrep` in `sạch`).
     - `agent-brain status "dự án nháp"` hiển thị run là `killed`.
     - Dọn dẹp:
       ```bash
       git -C /mnt/data/IDE/du-an-nhap checkout -f main
       git -C /mnt/data/IDE/du-an-nhap branch -D task/p203-a
       rm -f /mnt/data/IDE/du-an-nhap/.plan.md
       ```
  2. **Kịch bản B (Ctrl+C hai lần nhanh)**:
     - Chạy:
       ```bash
       AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'" --name p203-b
       ```
     - Khi thấy dòng log `tool Bash python3 -c 'import time; time.sleep(307)'`, ghi nhận các PID `<claude>,<bash>,<python>`.
     - Nhấn `Ctrl+C` hai lần liên tiếp thật nhanh (< 1 giây).
     - Chờ trong vòng 5 giây (`sleep 5`), kiểm tra các PID:
       ```bash
       ps -p <claude>,<bash>,<python> -o pid,args
       pgrep -af '^python3 -c import time; time.sleep\(307\)' || echo sạch
       ```
       Toàn bộ nhóm tiến trình lập tức bị ngắt (lệnh `ps` chỉ in tiêu đề `PID ARGS`, `pgrep` in `sạch`).
     - Dọn dẹp:
       ```bash
       git -C /mnt/data/IDE/du-an-nhap checkout -f main
       git -C /mnt/data/IDE/du-an-nhap branch -D task/p203-b
       rm -f /mnt/data/IDE/du-an-nhap/.plan.md
       ```
  3. **Kịch bản C (Đóng cửa sổ terminal)**:
     - Mở một cửa sổ terminal mới, chạy:
       ```bash
       AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'" --name p203-c
       ```
     - Khi `python3 -c 'import time; time.sleep(307)'` đang chạy, ghi nhận các PID `<claude>,<bash>,<python>`, sau đó đóng hẳn cửa sổ terminal đó (gửi SIGHUP).
     - Tại terminal còn lại, kiểm tra các PID sau 5 giây:
       ```bash
       ps -p <claude>,<bash>,<python> -o pid,args
       pgrep -af '^python3 -c import time; time.sleep\(307\)' || echo sạch
       ```
       Toàn bộ tiến trình biến mất trong vòng 5 giây (lệnh `ps` chỉ in tiêu đề `PID ARGS`, `pgrep` in `sạch`).
     - Dọn dẹp:
       ```bash
       git -C /mnt/data/IDE/du-an-nhap checkout -f main
       git -C /mnt/data/IDE/du-an-nhap branch -D task/p203-c
       rm -f /mnt/data/IDE/du-an-nhap/.plan.md
       ```
- **Ghi nhận**: Điền `Đạt` vào dòng `P2-03` trong `protocol-log/nghiem-thu-p2.md`.

---

### P2-07: Chạy lệnh `inspect`
- **Mục tiêu**: Kiểm tra lệnh `inspect` chỉ tốn đúng 1 turn, đọc diff chưa commit và không để lại session file lưu rác.
- **Tiền điều kiện**: Dự án `/mnt/data/IDE/du-an-nhap` ở nhánh `main`.
- **Lệnh thực hiện**:
  1. Tạo sửa đổi chưa commit trên file `src/calc.py`:
     ```bash
     echo "# test uncommitted diff" >> /mnt/data/IDE/du-an-nhap/src/calc.py
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
  git -C /mnt/data/IDE/du-an-nhap checkout -- src/calc.py
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

---

## 3. Nghiệm thu dashboard (Phase 5 / P4-01..P4-10)

Quy trình nghiệm thu các chức năng của giao diện web nhúng `agent-brain serve`. Kết quả được người dùng ghi nhận vào bảng tổng hợp tại [`protocol-log/nghiem-thu-p4.md`](../protocol-log/nghiem-thu-p4.md).

> [!CAUTION]
> **Nhắc nhở an toàn tiến trình**: Các tiến trình agent con chỉ được dừng thông qua nút **Kill** trên giao diện dashboard, tổ hợp phím **Ctrl+C** trên terminal hoặc đóng cửa sổ terminal. Tuyệt đối không dùng các lệnh dừng tiến trình hàng loạt theo tên. Kiểm tra tiến trình bằng `ps -p` với danh sách PID cụ thể đã ghi nhận.

---

### P4-01: Chạy binary và mở dashboard
- **Mục tiêu**: Mở được dashboard trong trình duyệt từ lệnh `agent-brain serve` mà không cần cài đặt thêm bất kỳ phần mềm hay runtime nào, tất cả 5 màn hình tải đầy đủ và đúng kiểu dáng.
- **Tiền điều kiện**: Dự án `/mnt/data/IDE/du-an-nhap` ở nhánh `main` sạch sẽ.
- **Lệnh thực hiện**:
  ```bash
  agent-brain serve
  ```
- **Quan sát & Kết quả mong đợi**:
  1. Terminal in dòng thông báo khởi động kèm URL chứa token:
     `agent-brain dashboard: http://127.0.0.1:7437/?token=TOKEN`
  2. Mở URL trên trong trình duyệt: server tự chuyển hướng `303 See Other` về `/`, cấp cookie phiên `agent_brain_token` và hiển thị màn hình **Tổng quan** (`#/`).
  3. Lần lượt bấm chuyển qua 4 màn hình còn lại trên thanh điều hướng:
     - **Giao việc** (`#/assign`)
     - **Chi tiết task** (`#/task/...`)
     - **Thống kê** (`#/stats`)
     - **Dự án** (`#/projects`)
  4. Mọi màn hình tải nhanh chóng, hiển thị đúng stylesheet Tailwind CSS ngoại tuyến, không lỗi bố cục, không có thông báo vi phạm CSP trong bảng điều khiển (Console) của trình duyệt.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-01` trong `protocol-log/nghiem-thu-p4.md`.
- **Dọn dẹp**: Nhấn Ctrl+C trên terminal để dừng `serve`.

---

### P4-02: Truy cập từ máy khác trong mạng LAN
- **Mục tiêu**: Đảm bảo dashboard chỉ lắng nghe cục bộ trên `127.0.0.1`, không thể truy cập từ thiết bị khác trong mạng LAN.
- **Lệnh thực hiện**:
  1. Khởi động dashboard trên máy chạy:
     ```bash
     agent-brain serve
     ```
  2. Kiểm tra cổng lắng nghe tại terminal:
     ```bash
     ss -ltnp | grep 7437
     ```
  3. Từ một máy tính khác trong cùng mạng LAN (hoặc dùng một terminal riêng thử kết nối tới IP LAN của máy này):
     ```bash
     curl -m 5 http://<IP LAN của máy này>:7437/
     ```
- **Kết quả mong đợi**:
  - Lệnh `ss` chỉ hiển thị địa chỉ cục bộ `127.0.0.1:7437` (hoặc `127.0.0.1:PORT`), tuyệt đối không có `0.0.0.0:7437` hay `*:7437`.
  - Lệnh `curl` từ máy ngoài thất bại với lỗi `Connection refused` hoặc quá thời gian chờ (timeout).
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-02` trong `protocol-log/nghiem-thu-p4.md`.
- **Dọn dẹp**: Nhấn Ctrl+C để dừng `serve`.

---

### P4-03: Truy cập không có token
- **Mục tiêu**: Xác thực cơ chế phòng vệ chặn truy cập khi thiếu token hoặc cookie phiên (mã 401).
- **Lệnh thực hiện**:
  1. Khởi động: `agent-brain serve`.
  2. Mở một cửa sổ trình duyệt ẩn danh (Incognito/Private) và truy cập thẳng địa chỉ:
     `http://127.0.0.1:7437/` (không truyền tham số `?token=`).
  3. Dừng server trên terminal bằng Ctrl+C, sau đó khởi động lại bằng `agent-brain serve` (server sinh token mới ngẫu nhiên). Quay lại tab trình duyệt cũ đã từng đăng nhập thành công trước đó và bấm F5 tải lại trang.
- **Kết quả mong đợi**:
  - Tại bước 2: Trình duyệt nhận trang lỗi `401 Unauthorized` với thông điệp: "Thiếu hoặc sai token — mở URL có token in ở terminal chạy agent-brain serve".
  - Tại bước 3: Tab cũ nhận mã 401 do cookie phiên từ lần chạy trước không còn hiệu lực.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-03` trong `protocol-log/nghiem-thu-p4.md`.
- **Dọn dẹp**: Dừng `serve` bằng Ctrl+C.

---

### P4-04: Giao việc từ UI
- **Mục tiêu**: Kiểm tra quy trình giao việc bằng tiếng Việt từ giao diện web, theo dõi dòng sự kiện thời gian thực và chuyển tiếp các bước Lead → Dev → QA.
- **Tiền điều kiện**: Repo `/mnt/data/IDE/du-an-nhap` ở nhánh `main` sạch sẽ.
- **Lệnh thực hiện**:
  1. Khởi động: `agent-brain serve`. Mở URL được cấp trên trình duyệt.
  2. Chuyển sang màn hình **Giao việc** (`#/assign`).
  3. Chọn dự án `dự án nháp` trong danh sách tìm kiếm gợi ý.
  4. Nhập nội dung yêu cầu tiếng Việt:
     `thêm hàm tính bình phương vào src/calc.py`
  5. Bấm nút **Giao việc cho Lead**.
  6. Giao diện tự động chuyển tới màn hình **Chi tiết task**:
     - Quan sát hộp log hiển thị các sự kiện trực tiếp từ Claude qua SSE.
     - Bước Lead hiển thị trạng thái `đang chạy`, sau khi hoàn thành chuyển sang `xong`.
     - Xuất hiện biểu ngữ (banner) trạng thái `Chờ duyệt plan` và nội dung file kế hoạch `.plan.md` ở chế độ chỉ đọc.
  7. Bấm nút **Chạy Dev**: Quan sát bước Dev chuyển sang `đang chạy`, sau khi thực hiện xong chuyển thành `dev_done`.
  8. Bấm nút **Chạy QA**: Quan sát bước QA thực thi kiểm thử và chuyển thành Đạt (`qa_pass`).
- **Kết quả mong đợi**: Pipeline hoàn thành đầy đủ 3 bước qua các thao tác bấm nút trên web, trạng thái cập nhật tự động theo thời gian thực.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-04` trong `protocol-log/nghiem-thu-p4.md`.
- **Dọn dẹp**:
  ```bash
  git -C /mnt/data/IDE/du-an-nhap checkout -f main
  git -C /mnt/data/IDE/du-an-nhap branch -D task/them-ham-tinh-binh-phuong-vao-srccalcpy || true
  ```

---

### P4-05: Reload trang giữa chừng
- **Mục tiêu**: Đảm bảo khi tải lại trang web lúc task đang chạy, hệ thống đọc lại toàn bộ nhật ký sự kiện cũ từ SQLite và tiếp tục nhận các sự kiện mới qua kết nối SSE mà không bị gián đoạn.
- **Lệnh thực hiện**:
  1. Trong khi một bước của task đang chạy và log đang hiển thị trực tiếp (ví dụ khi Lead hoặc Dev đang xử lý ở P4-04), nhấn phím F5 hoặc nút Reload của trình duyệt.
- **Kết quả mong đợi**: Trang web tải lại tức thì, toàn bộ các dòng sự kiện trước đó được nạp lại đầy đủ từ SQLite qua API `/api/runs/:id/events`, sau đó kết nối SSE tự động tái thiết lập và các dòng log mới tiếp tục hiển thị nối tiếp mượt mà.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-05` trong `protocol-log/nghiem-thu-p4.md`.

---

### P4-06: Bấm Kill từ UI (cây tiến trình sleep 307)
- **Mục tiêu**: Đảm bảo nút Kill trên giao diện ngắt sạch toàn bộ cây tiến trình con (Claude CLI, bash và lệnh con chạy nền), không để sót bất kỳ tiến trình mồ côi nào (tương tự kiểm thử P2-02).
- **Chuẩn bị cấu hình wrapper**:
  ```bash
  python3 -c "import json, os; c = json.load(open('agent-brain/assets/config.default.json')); c['claude_bin'] = os.path.abspath('scripts/live/claude-allow-sleep.sh'); json.dump(c, open('/tmp/ab-live.json', 'w'), indent=2)"
  ```
- **Lệnh thực hiện (Ca 1 — Kill tác vụ khởi tạo từ UI)**:
  1. Khởi động dashboard với cấu hình wrapper:
     ```bash
     AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain serve
     ```
  2. Mở dashboard trên trình duyệt, vào màn hình **Giao việc**, chọn `dự án nháp`, nhập yêu cầu giữ tiến trình:
     `Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'`
     với tên slug `p406-kill`.
  3. Bấm nút **Giao việc cho Lead**.
  4. Khi thấy dòng sự kiện `tool Bash python3 -c 'import time; time.sleep(307)'` xuất hiện trên khung log của UI, mở một cửa sổ terminal khác và kiểm tra cây tiến trình:
     ```bash
     ps -eo pid,ppid,pgid,sid,args --forest | grep -A3 '[a]gent-brain'
     ```
     Ghi lại chính xác 3 PID: tiến trình `claude`, tiến trình `bash` và tiến trình `python3`.
  5. Quay lại trình duyệt, nhấp nút **Kill** màu đỏ trên màn hình Chi tiết task.
  6. Chờ 5 giây (`sleep 5`), sau đó kiểm tra lại các PID đã ghi nhận tại terminal:
     ```bash
     ps -p <claude>,<bash>,<python> -o pid,args
     ```
- **Lệnh thực hiện (Ca 2 — Kill tác vụ khởi tạo từ terminal thông qua UI)**:
  1. Giữ dashboard đang chạy. Mở một terminal khác và khởi chạy task:
     ```bash
     AGENT_BRAIN_CONFIG=/tmp/ab-live.json agent-brain run lead "dự án nháp" "Trước khi làm gì khác, chạy đúng lệnh bash này ở foreground (KHÔNG dùng run_in_background) và chờ nó xong: python3 -c 'import time; time.sleep(307)'" --name p406-term-kill
     ```
  2. Trên giao diện dashboard, vào màn hình **Tổng quan** hoặc **Chi tiết task**, thấy task mới xuất hiện và đang chạy.
  3. Ghi lại các PID của tác vụ này bằng `ps -eo pid,ppid,pgid,sid,args --forest`.
  4. Bấm nút **Kill** trên giao diện dashboard.
  5. Sau 5 giây, kiểm tra lại danh sách PID.
- **Kết quả mong đợi**:
  - Ở cả hai ca, lệnh `ps -p` chỉ in duy nhất dòng tiêu đề `PID ARGS`, toàn bộ các tiến trình liên quan đã được dọn sạch hoàn toàn.
  - Trên giao diện UI, bước thực thi chuyển sang trạng thái `killed`.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-06` trong `protocol-log/nghiem-thu-p4.md`.
- **Dọn dẹp**:
  ```bash
  git -C /mnt/data/IDE/du-an-nhap checkout -f main
  git -C /mnt/data/IDE/du-an-nhap branch -D task/p406-kill || true
  git -C /mnt/data/IDE/du-an-nhap branch -D task/p406-term-kill || true
  rm -f /mnt/data/IDE/du-an-nhap/.plan.md
  ```

---

### P4-07: Tên dự án mơ hồ trên UI
- **Mục tiêu**: Kiểm tra hàng rào chống mơ hồ (Ambiguity gate) trên giao diện Giao việc: không tự ý chạy khi có nhiều hơn một dự án trùng khớp.
- **Chuẩn bị dữ liệu thử nghiệm**:
  Mở file `~/.config/agent-brain/projects_map.json` và tạm thời thêm alias `"nháp"` vào một dự án thứ hai khác ngoài `dự án nháp`.
- **Lệnh thực hiện**:
  1. Mở màn hình **Giao việc** (`#/assign`).
  2. Gõ chữ `"nháp"` vào ô tìm kiếm Dự án.
- **Quan sát & Kết quả mong đợi**:
  Hộp gợi ý xổ xuống hiển thị cả 2 ứng viên cùng có alias `"nháp"`. Hệ thống hiển thị cảnh báo mơ hồ và giữ nút giao việc ở trạng thái vô hiệu hóa. Tác vụ chỉ có thể được tạo khi người dùng nhấp chuột chọn đích danh một dự án trong danh sách.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-07` trong `protocol-log/nghiem-thu-p4.md`.
- **Dọn dẹp**: Xóa bỏ alias tạm thời trong `projects_map.json` để trả về trạng thái chuẩn.

---

### P4-08: Đổi profile
- **Mục tiêu**: Kiểm tra cơ chế lọc tầm nhìn theo hồ sơ (`Tất cả`, `work`, `personal`) và tính bất biến của profile dự án khi giao việc mới.
- **Lệnh thực hiện**:
  1. Trên thanh điều hướng, lần lượt bấm chọn bộ lọc `Tất cả`, `work` và `personal`.
  2. Quan sát danh sách task tại trang **Tổng quan**, số liệu tại trang **Thống kê** và danh sách gợi ý tại trang **Giao việc**.
  3. Khi đang chọn bộ lọc `personal`, vào trang **Giao việc** và chọn `dự án nháp` (vốn có cấu hình `profile: "work"` trong `projects_map.json`), khởi tạo một task mới.
- **Quan sát & Kết quả mong đợi**:
  - Các màn hình chỉ hiển thị đúng các mục tương ứng với profile đã chọn.
  - Task mới tạo cho `dự án nháp` vẫn được thực thi bằng profile `work` (biến `CLAUDE_CONFIG_DIR` không bị ép sang `~/.claude-orca`), minh chứng rằng bộ chọn trên UI chỉ là bộ lọc giao diện chứ không ghi đè profile cấu hình của dự án.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-08` trong `protocol-log/nghiem-thu-p4.md`.

---

### P4-09: Tắt mạng internet
- **Mục tiêu**: Đảm bảo toàn bộ tài nguyên của dashboard (HTML, CSS, JS) hoạt động 100% ngoại tuyến, không bị phụ thuộc vào kết nối internet.
- **Lệnh thực hiện**:
  1. Ngắt kết nối mạng (rút cáp mạng, tắt Wi-Fi hoặc chạy `nmcli networking off`).
  2. Tải lại và chuyển đổi giữa tất cả các màn hình của dashboard (`#/`, `#/assign`, `#/task/...`, `#/stats`, `#/projects`).
  3. Bật lại mạng (`nmcli networking on`).
- **Quan sát & Kết quả mong đợi**:
  Tất cả các màn hình, phông chữ, bảng màu Tailwind và biểu đồ thanh CSS hiển thị hoàn hảo, không có bất kỳ yêu cầu mạng nào bị thất bại hay giao diện bị vỡ kiểu dáng.
- **Ghi nhận**: Điền `Đạt` vào dòng `P4-09` trong `protocol-log/nghiem-thu-p4.md`.

---

### P4-10: So sánh UI với thiết kế (nếu có Figma)
- **Mục tiêu**: Đối chiếu với bản thiết kế Figma gốc.
- **Kết quả**: Không áp dụng. Căn cứ theo quyết định thiết kế D-18, dự án không xây dựng bản thiết kế Figma trước, giao diện được phát triển theo đặc tả UI-SPEC trực tiếp.
- **Ghi nhận**: Điền `Không áp dụng` vào dòng `P4-10` trong `protocol-log/nghiem-thu-p4.md` kèm ghi chú `D-18: chưa có thiết kế Figma`.

