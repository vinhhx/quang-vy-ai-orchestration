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
  5. Bấm nút **Giao việc**.
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
  3. Bấm nút **Giao việc**.
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

---

## 4. Nghiệm thu Phase 5.2 (AUTO-07)

Nghiệm thu quy trình (workflow) và autopilot với `claude` thật trên dự án nháp `/mnt/data/IDE/du-an-nhap` (profile `work`). Mọi ca chạy trong một môi trường cô lập: binary release dựng từ repo này, một bản sao của `brain.db` thật, thư mục cấu hình và dữ liệu riêng, dashboard ở cổng 7438. `brain.db` thật, `~/.config/agent-brain` và `agent-brain serve` đang chạy của bạn không bị đụng tới. Kết quả ghi vào bảng tổng hợp [`protocol-log/nghiem-thu-p5-2.md`](../protocol-log/nghiem-thu-p5-2.md).

Chi phí dự kiến (giá tương đương API): P5.2-01 khoảng $2–5, P5.2-02 một lượt Lead (khoảng $0.1–0.3), P5.2-03 một lượt Lead và một phần Review plan.

> [!CAUTION]
> **An toàn tiến trình và tài khoản**:
> - Chỉ dừng tiến trình bằng `$AB kill <task_id>`, Ctrl+C trong terminal của chính tiến trình đó, hoặc `kill -INT <PID>` với PID đã ghi lại. Không dùng `pkill`, `killall` hay bất kỳ lệnh dừng tiến trình theo tên nào.
> - `ps` chỉ dùng để đọc (`ps -eo ... --forest`, `ps -p <PID,...>`, `ps --ppid <PID>`, `ps -eo pid,ppid,pgid,args | awk -v g=<PGID> '$3 == g'`). Không tìm process group bằng cờ `-g` của `ps`: với procps, cờ -g chọn theo session chứ không theo process group.
> - Không chạy task nào trên profile `personal` khi `~/.claude-orca` đang được dùng. Mọi ca dưới đây chạy trên `du-an-nhap` (profile `work`).
> - Không dùng cổng của dashboard thật; dashboard của đợt nghiệm thu này chạy ở `--port 7438`. Không giao việc cho `du-an-nhap` từ dashboard thật trong lúc nghiệm thu (dashboard thật không thấy bản sao DB nên không chặn được việc chạy chồng).

---

### P5.2-00: Chuẩn bị môi trường cô lập
- **Mục tiêu**: Dựng binary release, sao chép `brain.db` thật sang một thư mục nháp, tạo cấu hình riêng có hai quy trình nghiệm thu, và chứng minh bản nâng cấp schema v1 → v2 giữ nguyên dữ liệu thật.
- **Tiền điều kiện**:
  - `/mnt/data/IDE/du-an-nhap` đang ở nhánh `main` và sạch (`git -C /mnt/data/IDE/du-an-nhap status --short --branch` chỉ in `## main`).
  - Trong `~/.config/agent-brain/projects_map.json`, dự án `du-an-nhap` trỏ tới `/mnt/data/IDE/du-an-nhap` với `profile: "work"`.
  - `claude --version` chạy được với tài khoản `work` (`~/.claude`).
- **Chuẩn bị**:
  1. Dựng binary và tạo thư mục nháp (chạy từ gốc repo):
     ```bash
     cd /mnt/data/IDE/QuangVy-Orchestration
     cargo build --release --manifest-path agent-brain/Cargo.toml --offline --locked
     AB="$PWD/agent-brain/target/release/agent-brain"
     LIVE="$(mktemp -d /tmp/p52-live.XXXXXX)"
     mkdir -p "$LIVE/xdg/agent-brain" "$LIVE/data"
     ```
  2. Sao chép `brain.db` thật bằng API backup của SQLite (không dùng `cp`: `serve` của bạn có thể đang ghi). `mode=rw` không tạo file mới nếu đường dẫn sai:
     ```bash
     python3 - "$HOME/.local/share/agent-brain/brain.db" "$LIVE/brain.db" <<'EOF'
     import sqlite3, sys
     src = sqlite3.connect(f"file:{sys.argv[1]}?mode=rw", uri=True, timeout=10)
     dst = sqlite3.connect(sys.argv[2])
     src.backup(dst)
     dst.close()
     src.close()
     print("backup xong:", sys.argv[2])
     EOF
     chmod 600 "$LIVE/brain.db"
     ```
  3. Ghi file môi trường dùng chung cho mọi terminal của đợt nghiệm thu (`XDG_DATA_HOME` riêng để thư mục `runs/` và `serve.lock` nằm trong `$LIVE`; `q` đọc bản sao DB, `counts` đếm số dòng):
     ```bash
     cat > "$LIVE/env.sh" <<EOF
     export LIVE="$LIVE"
     export AB="$AB"
     export AGENT_BRAIN_DB="$LIVE/brain.db"
     export XDG_CONFIG_HOME="$LIVE/xdg"
     export XDG_DATA_HOME="$LIVE/data"
     unset AGENT_BRAIN_CONFIG
     q() { python3 -c 'import sqlite3, sys; c = sqlite3.connect(sys.argv[1], timeout=10); [print(*r, sep=" | ") for r in c.execute(sys.argv[2], [int(a) if a.isdigit() else a for a in sys.argv[3:]])]' "\$AGENT_BRAIN_DB" "\$@"; }
     counts() { q "PRAGMA user_version"; for t in projects tasks runs events; do q "SELECT '\$t', COUNT(*) FROM \$t"; done; }
     EOF
     source "$LIVE/env.sh"
     ```
     Mỗi terminal mới của đợt nghiệm thu chạy `source <đường dẫn $LIVE>/env.sh` trước tiên.
  4. Chép danh mục dự án (và `config.json` nếu bạn có) sang cấu hình riêng, rồi ghi `workflows.json` của đợt nghiệm thu:
     ```bash
     cp ~/.config/agent-brain/projects_map.json "$XDG_CONFIG_HOME/agent-brain/projects_map.json"
     if [ -f ~/.config/agent-brain/config.json ]; then cp ~/.config/agent-brain/config.json "$XDG_CONFIG_HOME/agent-brain/config.json"; fi
     cat > "$XDG_CONFIG_HOME/agent-brain/workflows.json" <<'EOF'
     {
       "p52-full": {
         "stages": ["plan", "plan-review", "gate:you", "implement", "code-review", "qa", "fix"],
         "loops": { "plan": "plan-review", "implement": "code-review", "qa": "fix" },
         "budget_usd": 5,
         "auto": true
       },
       "chi-phi-thap": {
         "stages": ["plan", "plan-review", "gate:you", "implement", "qa"],
         "budget_usd": 0.05,
         "auto": true
       }
     }
     EOF
     ```
     `p52-full` là `mac-dinh` với ngân sách $5; `chi-phi-thap` có ngân sách $0.05 để dừng sau bước đầu tiên. Cả hai đều `auto: true`.
- **Lệnh thực hiện** (nâng cấp schema trên dữ liệu thật; `stats --json` phải là lệnh đầu tiên của binary mới mở bản sao):
  ```bash
  counts                       # trước: user_version 1 và số dòng projects / tasks / runs / events
  "$AB" stats --json > "$LIVE/stats.json"; echo "exit=$?"
  counts                       # sau
  git -C /mnt/data/IDE/du-an-nhap status --short --branch
  "$AB" status du-an-nhap      # đường dẫn nhanh, 0 token
  ```
- **Kết quả mong đợi**:
  1. `stats --json` thoát `0`.
  2. `user_version` đổi từ `1` sang `2`. Nếu bản sao đã là `2` (DB thật đã được binary mới nâng cấp trước đó), ghi chú điều này: phép thử nâng cấp trên dữ liệu thật không áp dụng.
  3. Số dòng `tasks`, `runs`, `events` trước và sau **bằng nhau**. `projects` bằng hoặc lớn hơn (binary đồng bộ bảng `projects` từ `projects_map.json` mỗi lần mở).
  4. `du-an-nhap` ở nhánh `main`, sạch; `status` in đúng đường dẫn `/mnt/data/IDE/du-an-nhap`.
- **Ghi nhận**: Điền mục `Môi trường (P5.2-00)` ở đầu `protocol-log/nghiem-thu-p5-2.md`: commit của repo, `claude --version`, `user_version` và số dòng trước / sau.
- **Dọn dẹp**: Không. Giữ terminal này (đã `source env.sh`) cho các ca sau.

---

### P5.2-01: mac-dinh tới QA đạt với ít nhất một vòng sửa lại plan
- **Mục tiêu**: Chạy trọn vòng của quy trình đầy đủ với `claude` thật: Review plan yêu cầu sửa lại ít nhất một lần, Lead sửa plan theo báo cáo, autopilot dừng ở cổng duyệt, bạn duyệt, rồi Code → Review code → QA đạt. Ghi chính xác id run, vai trò, giai đoạn, vòng, kết luận và chi phí (AUTO-07).
- **Tiền điều kiện**: P5.2-00 xong; `du-an-nhap` ở `main`, sạch.
- **Lệnh thực hiện**:
  1. Tạo task bằng Lead chạy tay với quy trình `p52-full`:
     ```bash
     git -C /mnt/data/IDE/du-an-nhap checkout main
     "$AB" run lead --workflow p52-full --name p52-loop -- du-an-nhap "Thêm hàm giai_thua(n) và test cho nó"; echo "exit=$?"
     ID=$(q "SELECT id FROM tasks WHERE branch = 'task/p52-loop' ORDER BY id DESC LIMIT 1"); echo "task $ID"
     q "SELECT json_extract(workflow_json, '\$.name') FROM tasks WHERE id = ?" "$ID"    # p52-full
     ```
  2. Cài một lỗi rõ ràng vào plan: mở `/mnt/data/IDE/du-an-nhap/.plan.md` bằng editor, thêm vào bảng `## Thay đổi` một dòng nêu file không tồn tại, ví dụ:
     `| 9 | src/khong_ton_tai.py | toàn bộ | gọi giai_thua từ file này | cần cho yêu cầu |`
     rồi commit với đúng message plan:
     ```bash
     git -C /mnt/data/IDE/du-an-nhap add .plan.md
     git -C /mnt/data/IDE/du-an-nhap commit -m "p52-loop: plan"
     ```
  3. Bật autopilot cho task (bắt đầu từ Review plan):
     ```bash
     "$AB" auto "$ID"; echo "exit=$?"
     ```
  4. Khi autopilot dừng ở cổng duyệt, làm kiểm tra giao diện UI52-03 của P5.2-04 (nút ở thẻ Bạn), rồi duyệt plan từ terminal (không bấm nút trên dashboard):
     ```bash
     "$AB" approve "$ID"; echo "exit=$?"
     ```
  5. Đọc kết quả từ bản sao DB:
     ```bash
     q "SELECT id, role, stage, round, outcome, cost_usd, usage_source FROM runs WHERE task_id = ? ORDER BY id" "$ID"
     q "SELECT auto_state, approved_commit FROM tasks WHERE id = ?" "$ID"
     git -C /mnt/data/IDE/du-an-nhap log --oneline main..task/p52-loop
     ```
- **Kết quả mong đợi**:
  1. Bước 1 thoát `0`; task lưu quy trình `p52-full`. (`claude` vẫn dùng tài khoản `work` ở `~/.claude`; các biến `XDG_*` chỉ đổi chỗ `agent-brain` đọc cấu hình và ghi dữ liệu. Nếu Lead báo chưa đăng nhập, dừng đợt nghiệm thu và ghi lại.)
  2. Bước 3 in lần lượt `Autopilot · task N · plan-review vòng 1`, `Autopilot · task N · plan vòng 2`, `Autopilot · task N · plan-review vòng 2` (có thể thêm vòng, tối đa 3), rồi `Autopilot dừng ở bước duyệt plan (task N) — duyệt: agent-brain approve N` và `exit=0`. Plan vòng 2 không còn dòng `src/khong_ton_tai.py`.
  3. Bước 4 in `Đã duyệt plan <sha7> lúc HH:MM (task N).`, rồi `implement vòng 1`, `code-review vòng 1` (thêm `fix` / `code-review` nếu Review code yêu cầu sửa), `qa vòng 1`, và kết thúc bằng `Autopilot xong: QA đạt (task N)`, `exit=0`.
  4. Bảng run có ít nhất: `lead · plan · 1`, `plan-review · plan-review · 1 · review:revise`, `lead · plan · 2`, `plan-review · plan-review · 2 · review:approve`, `dev · implement · 1`, `code-review · code-review · 1 · review:pass` (hoặc `review:revise` kèm `dev · fix` và một lần review sau), `qa · qa · 1 · qa:pass`; `usage_source` là `result` và `cost_usd` lớn hơn 0 cho mọi run. `auto_state = done`.
  5. Nhánh có các commit `p52-loop: plan` và các commit báo cáo review / QA.
  - Nếu Review plan duyệt ngay bản plan có lỗi (không có vòng sửa lại): đây là ghi chú của nghiệm thu, không phải lỗi phần mềm. Về `main`, lặp lại từ bước 1 với `--name p52-loop2` và một lỗi nặng hơn (thêm cả dòng xóa `src/calc.py`, trái với yêu cầu), rồi ghi cả hai lần vào Ghi chú.
  - Nếu autopilot dừng vì lý do khác (ví dụ `hết ngân sách`), ghi nguyên văn dòng `Autopilot dừng · …`; được chạy tiếp một lần bằng `"$AB" auto "$ID" --more-round` và ghi lại điều đó.
- **Ghi nhận**: Dòng `P5.2-01`: dán nguyên văn kết quả câu SQL ở bước 5 (id run, vai trò, giai đoạn, vòng, kết luận, `cost_usd`, `usage_source`) và tổng chi phí; không làm tròn hay tóm tắt lại số.
- **Dọn dẹp**: Chưa xóa nhánh (xem mục Dọn dẹp cuối phần này). Chuyển về `main` trước ca sau: `git -C /mnt/data/IDE/du-an-nhap checkout main`.

---

### P5.2-02: Dừng do ngân sách
- **Mục tiêu**: Autopilot dừng với `hết ngân sách` ngay sau run đầu tiên khi tổng chi phí vượt giới hạn của quy trình, và không bắt đầu bước nào nữa (biên của AUTO-07).
- **Tiền điều kiện**: P5.2-00 xong; `du-an-nhap` ở `main`, sạch.
- **Lệnh thực hiện**:
  ```bash
  git -C /mnt/data/IDE/du-an-nhap checkout main
  "$AB" auto --start --workflow chi-phi-thap --name p52-budget -- du-an-nhap "Thêm docstring cho hàm tinh_tong"; echo "exit=$?"
  ID2=$(q "SELECT id FROM tasks WHERE branch = 'task/p52-budget' ORDER BY id DESC LIMIT 1"); echo "task $ID2"
  q "SELECT auto_state, json_extract(auto_json, '\$.stop.reason') FROM tasks WHERE id = ?" "$ID2"
  q "SELECT id, role, stage, round, outcome, cost_usd, usage_source FROM runs WHERE task_id = ? ORDER BY id" "$ID2"
  q "SELECT COUNT(*) FROM runs WHERE task_id = ? AND role = 'plan-review'" "$ID2"
  ```
- **Kết quả mong đợi**:
  1. Terminal in `Autopilot · task mới p52-budget · plan vòng 1`, log của Lead, rồi đúng một dòng `Autopilot dừng · hết ngân sách: Đã dùng M / 60 phút · $X / $0.05. Run đang chạy đã xong; autopilot không bắt đầu bước nào nữa. (task N)` và `exit=4`.
  2. `auto_state = stopped`, `auto_json.stop.reason = het_ngan_sach`.
  3. Đúng một run (`lead · plan · 1`), không có run `plan-review` (đếm được `0`).
  - Nếu Lead tốn dưới $0.05 (ví dụ `config.json` của bạn đặt Lead dùng model rẻ) thì autopilot chạy thêm bước rồi mới dừng: ghi lại chi phí thật của Lead và bước dừng.
- **Ghi nhận**: Dòng `P5.2-02`: dòng terminal nguyên văn, mã thoát, kết quả ba câu SQL.
- **Dọn dẹp**: `git -C /mnt/data/IDE/du-an-nhap checkout main` (xóa nhánh ở mục Dọn dẹp cuối).

---

### P5.2-03: Kill giữa vòng lặp
- **Mục tiêu**: `agent-brain kill` trong lúc Review plan đang chạy dừng cả run lẫn autopilot, không sót tiến trình nào, và task dừng với lý do `dừng bởi bạn`.
- **Tiền điều kiện**: P5.2-00 xong; `du-an-nhap` ở `main`, sạch.
- **Lệnh thực hiện**:
  1. Terminal 1 (đã `source env.sh`):
     ```bash
     git -C /mnt/data/IDE/du-an-nhap checkout main
     "$AB" auto --start --workflow p52-full --name p52-kill -- du-an-nhap "Thêm dòng chữ “ẾCH ỘP ỮNG HỘ” vào cuối README"; echo "exit=$?"
     ```
  2. Terminal 2 (`source <đường dẫn $LIVE>/env.sh`): lặp lại câu sau cho tới khi có một dòng (Review plan đang chạy, sau bước Lead):
     ```bash
     q "SELECT t.id, t.auto_pid, r.id, r.pid, r.pgid FROM runs r JOIN tasks t ON t.id = r.task_id WHERE t.branch = 'task/p52-kill' AND r.role = 'plan-review' AND r.status = 'running'"
     ```
     Ghi lại: `ID3` (task), `DRIVER` (`auto_pid`, tiến trình `agent-brain auto`), `RUN` (id run), `CLAUDE` (`pid` của run) và `PGID`.
  3. Ghi lại cây tiến trình, chỉ đọc:
     ```bash
     ps -o pid,ppid,pgid,args --ppid "$DRIVER"      # tiến trình con agent-brain run plan-review (CHILD)
     ps -eo pid,ppid,pgid,args | awk -v g="$PGID" '$3 == g'   # claude và các tiến trình con của nó (cùng PGID)
     ps -eo pid,ppid,pgid,args --forest             # đối chiếu cây dưới DRIVER
     ```
     Ghi đủ danh sách PID: `DRIVER`, `CHILD`, `CLAUDE` và mọi tiến trình con của `CLAUDE` thấy được.
  4. Dừng task:
     ```bash
     "$AB" kill "$ID3"; echo "exit=$?"
     ```
  5. Chờ 5 giây (ví dụ `python3 -c 'import time; time.sleep(5)'`), rồi kiểm tra đúng các PID đã ghi và trạng thái trong DB:
     ```bash
     ps -p "<DRIVER>,<CHILD>,<CLAUDE>,<các PID con khác>" -o pid,args
     q "SELECT id, role, status FROM runs WHERE id = ?" "$RUN"
     q "SELECT auto_state, json_extract(auto_json, '\$.stop.reason') FROM tasks WHERE id = ?" "$ID3"
     q "SELECT COUNT(*) FROM runs WHERE status = 'running'"
     ```
- **Kết quả mong đợi**:
  1. Lệnh kill in `Đang dừng task N (run R, pid P)...`, `Đã dừng task N (run R) sau … ms` và `Autopilot của task N cũng dừng với lý do "dừng bởi bạn".`, thoát `0`.
  2. Terminal 1 in `Autopilot dừng · dừng bởi bạn: Bạn đã dừng autopilot lúc HH:MM. (task N)` và `exit=4`.
  3. `ps -p` chỉ in dòng tiêu đề `PID ARGS`.
  4. Run Review plan có `status = killed` (không phải `lost`); `auto_state = stopped`, lý do `dung_boi_ban`; không còn run nào `running` (đếm được `0`).
- **Ghi nhận**: Dòng `P5.2-03`: danh sách PID đã ghi, kết quả `ps -p` sau 5 giây, ba câu SQL, hai dòng terminal.
- **Dọn dẹp**: `git -C /mnt/data/IDE/du-an-nhap checkout -f main` (xóa nhánh ở mục Dọn dẹp cuối).

---

### P5.2-04: Giao diện (UI backstop)
- **Mục tiêu**: Kiểm tra bằng mắt sáu mục backstop của `05.2-UI-SPEC.md` (UI Considerations) trên dữ liệu thật của ba task nghiệm thu, ở 1024 / 768 / 480 px, giao diện sáng và tối.
- **Tiền điều kiện**: Môi trường P5.2-00. Nên mở dashboard ngay sau P5.2-00 và giữ suốt P5.2-01..03: UI52-03 cần lúc task `p52-loop` đang chờ ở cổng duyệt (bước 4 của P5.2-01), huy hiệu `Autopilot` màu xanh cần lúc autopilot đang chạy.
- **Lệnh thực hiện**:
  1. Terminal riêng (`source <đường dẫn $LIVE>/env.sh`), mở dashboard của bản sao ở cổng 7438 và ghi lại PID:
     ```bash
     "$AB" serve --port 7438 > "$LIVE/serve.out" 2>&1 &
     SERVE_PID=$!; echo "serve pid $SERVE_PID"
     cat "$LIVE/serve.out"     # agent-brain dashboard: http://127.0.0.1:7438/?token=TOKEN
     ```
     Mở URL đó trong Chrome. Đổi bề rộng bằng DevTools (Ctrl+Shift+M, nhập 1024, 768, 480); đổi sáng / tối bằng nút **Giao diện**. Chỉ xem: không bấm Giao việc, Duyệt plan, Trả về Lead hay các nút autopilot trên dashboard này.
  2. Kiểm tra từng mục (task: `p52-loop`, `p52-budget`, `p52-kill`):
     - **UI52-01** (flow ở 1024 / 768 px, task `p52-loop`): luồng Bắt đầu → plan ⇄ review → Bạn → code ⇄ review → QA ⇄ sửa nằm trên một hàng; ở 768 px luồng cuộn ngang bên trong vùng `Pipeline của task`, còn trang không cuộn ngang.
     - **UI52-02** (flow ở 480 px): các nhóm xếp dọc, mỗi nhóm hai thẻ; chân nhóm (`⟲ vòng N / 3`) và nút thẻ Bạn xuống dòng, trang không tràn ngang.
     - **UI52-03** (nút thẻ Bạn trong 112 px, 1024 px, lúc `p52-loop` chờ duyệt): `Duyệt plan` và `Trả về Lead` hiện đủ chữ (có thể xuống hai dòng, không bị cắt). Trong Console: `(b => b.scrollWidth <= b.clientWidth)(document.getElementById('ab-act-return_lead'))` trả `true`.
     - **UI52-04** (hàng tiêu đề Pipeline và huy hiệu autopilot ở 480 px): hàng `Pipeline` + huy hiệu + nút Bật / Dừng autopilot xuống dòng, chữ huy hiệu hiện đủ (xem trên `p52-loop` lúc đang chạy và trên `p52-budget` đã dừng).
     - **UI52-05** (độ tương phản sáng / tối): khung `ring-2` màu hổ phách của nhóm dừng và chữ lý do ở chân nhóm (`p52-budget`: `hết ngân sách`, `p52-kill`: `dừng bởi bạn`), huy hiệu `Autopilot` xanh nhấp nháy (task đang chạy và `◐ Autopilot` ở Tổng quan), màu nền của lựa chọn đang chọn trong ô Quy trình ở Giao việc (chọn thử `p52-full`, không bấm Giao việc): đọc rõ ở cả hai giao diện.
     - **UI52-06** (dấu tiếng Việt `Ế Ộ Ữ`): tiêu đề `p52-kill` ở Tổng quan và Chi tiết task, các dòng kết quả của thẻ và huy hiệu: dấu ở dòng trên cùng và dòng cuối không bị cắt, ở 1024 và 480 px.
  3. Dừng dashboard: `kill -INT "$SERVE_PID"` (hoặc Ctrl+C nếu chạy ở foreground). Chạy `ps -p "$SERVE_PID" -o pid,args`: chỉ còn dòng tiêu đề.
- **Kết quả mong đợi**: Cả sáu mục đạt; trang không tràn ngang ở mọi bề rộng; mọi lý do dừng hiện bằng chữ, không chỉ bằng màu.
- **Ghi nhận**: Bảng `Kiểm tra giao diện` trong `protocol-log/nghiem-thu-p5-2.md`, dòng `UI52-01` … `UI52-06` (ghi bề rộng, giao diện sáng / tối và task đã xem).
- **Dọn dẹp**: Dashboard 7438 đã dừng ở bước 3.

---

### Dọn dẹp
1. Đưa repo nháp về `main` và xóa **đúng** các nhánh đợt này đã tạo:
   ```bash
   git -C /mnt/data/IDE/du-an-nhap checkout -f main
   git -C /mnt/data/IDE/du-an-nhap branch -D task/p52-loop
   git -C /mnt/data/IDE/du-an-nhap branch -D task/p52-budget
   git -C /mnt/data/IDE/du-an-nhap branch -D task/p52-kill
   git -C /mnt/data/IDE/du-an-nhap status --short
   ```
   Nếu đã lặp P5.2-01 thì xóa thêm `task/p52-loop2`. `status --short` phải không in gì; nếu còn file giao thức chưa theo dõi do đợt này tạo (`.plan.md`, `.plan-review.md`, `.code-review.md`, `.qa-report.md`) thì xóa đúng các file đó.
2. Kiểm tra không còn tiến trình của đợt nghiệm thu: `ps -p` với mọi PID đã ghi (driver, dashboard 7438) chỉ in dòng tiêu đề.
3. Giữ `$LIVE` (bản sao DB, log run, `stats.json`) cho tới khi đã ghi xong `protocol-log/nghiem-thu-p5-2.md`; sau đó xóa vì nó chứa bản sao dữ liệu thật: `rm -rf "$LIVE"`.
4. Không lệnh nào trong phần này ghi vào `~/.local/share/agent-brain` hay `~/.config/agent-brain`; `brain.db` thật chỉ được nâng cấp khi bạn cài binary mới.

---

## 5. Nghiệm thu Phase 5.3 (Telegram, TELE-07)

Nghiệm thu bot Telegram với **bot thật của bạn** trên `api.telegram.org` và **điện thoại của bạn**: ghép nối, tin chờ duyệt kèm file plan, duyệt từ điện thoại, các nút ở lần dừng kèm ghi chú, Kill có xác nhận, chat lạ bị bỏ qua, không gửi lại sau khi khởi động lại. Mọi ca chạy trong môi trường cô lập: thư mục cấu hình, dữ liệu và `brain.db` riêng trong `$LIVE`, dashboard cổng 7439, hai repo nháp mới tạo, và **fake claude** (`tests/fakes/fake_claude.py`) thay cho `claude` thật nên không tốn tiền. `brain.db` thật, `~/.config/agent-brain`, `~/.claude-orca` và `agent-brain serve` đang chạy của bạn (cổng 7437) không bị đụng tới. Kết quả ghi vào [`protocol-log/nghiem-thu-p5-3.md`](../protocol-log/nghiem-thu-p5-3.md).

Cần chuẩn bị: một bot mới tạo bằng @BotFather (`/newbot`), điện thoại có Telegram, và một nhóm Telegram nhỏ có bot (cho ca chat lạ). Thời gian khoảng 30–45 phút.

> [!CAUTION]
> **An toàn token, tiến trình và tài khoản**:
> - Token bot chỉ được **bạn** gõ / dán vào `agent-brain telegram setup` trong terminal của bạn. Không dán token vào chat với Claude, không in `$XDG_CONFIG_HOME/agent-brain/telegram.json` (`cat`, `less`, `jq`…), không chụp màn hình có token. Chỉ được xem quyền của file: `stat -c %a`.
> - Chat ID của bạn và ID của nhóm in ra ở terminal (setup, `telegram test`, dòng `bỏ qua tin từ chat lạ`) **không** được chép vào kết quả: ghi `đã ghép (ẩn)` và `nhóm (ẩn)`.
> - Chỉ dừng tiến trình bằng nút Kill / `$AB kill <task_id>`, Ctrl+C trong terminal của chính tiến trình đó, hoặc `kill -INT <PID>` với PID đã ghi lại. Không dùng `pkill`, `killall` hay lệnh dừng theo tên. Không gửi tín hiệu nào cho `serve` cổng 7437 của bạn.
> - `ps` chỉ dùng để đọc (`ps -p <PID,...>`, `ps --ppid <PID>`, `ps -eo pid,ppid,pgid,args`).
> - Mỗi bot chỉ được một nơi đọc (`getUpdates`): trong lúc nghiệm thu không chạy `serve` hay `telegram setup` nào khác với cùng bot.

---

### P5.3-00: Chuẩn bị môi trường cô lập
- **Mục tiêu**: Dựng binary release, môi trường riêng `$LIVE` (cấu hình, dữ liệu, `brain.db`, `config.json` dùng fake claude, hai repo nháp, bốn kịch bản), và chứng minh môi trường chạy được mà chưa cần điện thoại: `serve` cổng 7439 chạy, chưa có dòng `Telegram` nào (chưa ghép nối), `telegram test` từ chối gọn.
- **Tiền điều kiện**: cổng 7439 trống (`ss -ltn | grep :7439` không in gì); `curl` và `python3` có trong `PATH`.
- **Chuẩn bị** (chạy từ gốc repo; Claude dùng thư mục scratchpad của phiên làm `$LIVE`, chạy tay thì dùng `mktemp`):
  1. Dựng binary và tạo thư mục (quyền `0700`):
     ```bash
     cd /mnt/data/IDE/QuangVy-Orchestration
     cargo build --release --manifest-path agent-brain/Cargo.toml --offline --locked
     REPO="$PWD"; AB="$REPO/agent-brain/target/release/agent-brain"
     LIVE="$(mktemp -d /tmp/p53-live.XXXXXX)"      # hoặc <scratchpad của phiên>/p53-live
     (umask 077; mkdir -p "$LIVE/xdg/agent-brain" "$LIVE/data/agent-brain" "$LIVE/repos" "$LIVE/scenarios")
     ```
  2. Ghi `$LIVE/env.sh` (các biến cô lập và ba hàm trợ giúp: `q` đọc `brain.db` của đợt này, `use_sc` chọn kịch bản fake claude, `dash_post` gửi POST tới dashboard 7439 với token dashboard chỉ đi qua stdin của curl):
     ```bash
     cat > "$LIVE/env.sh" <<EOF
     export LIVE="$LIVE"
     export REPO="$REPO"
     export AB="$AB"
     export XDG_CONFIG_HOME="\$LIVE/xdg"
     export XDG_DATA_HOME="\$LIVE/data"
     export AGENT_BRAIN_DB="\$LIVE/data/agent-brain/brain.db"
     export AGENT_BRAIN_CONFIG="\$LIVE/config.json"
     export GIT_CONFIG_GLOBAL="\$LIVE/gitconfig"
     export GIT_CONFIG_NOSYSTEM=1
     export FAKE_CLAUDE_SCENARIO="\$LIVE/scenario.json"
     export FAKE_CLAUDE_RECORD="\$LIVE/claude-record.jsonl"
     unset AGENT_BRAIN_TELEGRAM_API AGENT_BRAIN_CURL AGENT_BRAIN_TELEGRAM_BACKOFF_MS
     q() { python3 -c 'import sqlite3, sys; c = sqlite3.connect(sys.argv[1], timeout=10); [print(*r, sep=" | ") for r in c.execute(sys.argv[2], [int(a) if a.isdigit() else a for a in sys.argv[3:]])]' "\$AGENT_BRAIN_DB" "\$@"; }
     use_sc() { cp "\$LIVE/scenarios/\$1.json" "\$LIVE/scenario.json" && rm -f "\$LIVE/scenario.json.state.json" && echo "kịch bản: \$1"; }
     dash_post() { local tok; tok=\$(sed -n 's/^agent-brain dashboard: .*token=\([0-9a-f]*\).*/\1/p' "\$LIVE/serve.out" | tail -1); printf 'header = "Cookie: agent_brain_token=%s"\n' "\$tok" | curl -q -sS -K - -H 'Origin: http://127.0.0.1:7439' -H 'Content-Type: application/json' --data-raw "\${2:-{\}}" "http://127.0.0.1:7439\$1"; echo; }
     EOF
     chmod 600 "$LIVE/env.sh"; source "$LIVE/env.sh"
     ```
     Mỗi terminal mới của đợt nghiệm thu (kể cả terminal của bạn ở P5.3-01) chạy `source <đường dẫn $LIVE>/env.sh` trước tiên. `AGENT_BRAIN_TELEGRAM_API` bị bỏ đặt: bot nói chuyện với `api.telegram.org` thật.
  3. `gitconfig` riêng và `config.json` = `agent-brain/assets/config.default.json` với `claude_bin` là đường dẫn tuyệt đối của fake claude và `kill_grace_secs` 1:
     ```bash
     printf '[user]\n  name = P53 Live\n  email = p53-live@example.com\n[commit]\n  gpgsign = false\n[init]\n  defaultBranch = main\n' > "$LIVE/gitconfig"
     python3 - "$REPO/agent-brain/assets/config.default.json" "$REPO/tests/fakes/fake_claude.py" "$LIVE/config.json" <<'PY'
     import json, os, sys
     cfg = json.load(open(sys.argv[1], encoding="utf-8"))
     cfg["claude_bin"] = os.path.realpath(sys.argv[2])
     cfg["kill_grace_secs"] = 1
     with open(sys.argv[3], "w", encoding="utf-8") as f:
         json.dump(cfg, f, ensure_ascii=False, indent=2)
     print("config.json:", cfg["claude_bin"], cfg["kill_grace_secs"])
     PY
     ```
  4. Hai repo nháp và danh mục dự án riêng (`demo-tele` không ghi `workflow` nên dùng mặc định `mac-dinh`; `demo-tele-nhanh` dùng `nhanh`; cả hai profile `work`):
     ```bash
     for r in demo-tele demo-tele-nhanh; do
       mkdir -p "$LIVE/repos/$r/src"
       git -C "$LIVE/repos/$r" init -q -b main
       printf 'def app():\n    return 1\n' > "$LIVE/repos/$r/src/app.py"
       printf '# %s\n\nRepo nháp cho nghiệm thu Telegram (P5.3).\n' "$r" > "$LIVE/repos/$r/README.md"
       git -C "$LIVE/repos/$r" add src/app.py README.md
       git -C "$LIVE/repos/$r" commit -q -m init
     done
     python3 - "$LIVE" <<'PY'
     import json, os, sys
     live = sys.argv[1]
     def entry(pid, workflow=None):
         e = {"id": pid, "path": os.path.join(live, "repos", pid), "name_vi": pid,
              "aliases": [], "type": "code", "repo": None, "profile": "work", "missing": False}
         if workflow:
             e["workflow"] = workflow
         return e
     data = {"scan_roots": [], "projects": [entry("demo-tele"), entry("demo-tele-nhanh", "nhanh")]}
     with open(os.path.join(live, "xdg", "agent-brain", "projects_map.json"), "w", encoding="utf-8") as f:
         json.dump(data, f, ensure_ascii=False, indent=2)
     PY
     ```
  5. Bốn kịch bản fake claude trong `$LIVE/scenarios/` (cùng dạng `by_role` với `agent-brain/tests/dashboard_autopilot.rs` và `agent-brain/tests/telegram_actions.rs`; mỗi vai trò lấy lần lượt các mục của nó, mục cuối được dùng lại):
     - `gate.json`: Lead viết plan `Sửa app() trả về 2` (bảng `Thay đổi`: `src/app.py`, `README.md`), Review plan `approve` → autopilot dừng ở cổng duyệt.
     - `full.json`: như `gate.json`, cộng Dev sửa hai file và commit `tele-gate: sửa app`, Review code `pass`, QA `Đạt`.
     - `plan-cap.json`: Review plan luôn `revise` (1 mục chặn) → `hết vòng` sau 3 / 3 vòng; Lead lập lại plan bằng một dòng nối thêm.
     - `dev-sleep.json`: Dev ngủ 600 giây, không sửa gì (một run chạy lâu để thử Kill).
     ```bash
     python3 - "$LIVE/scenarios" <<'PY'
     import json, os, sys
     out = sys.argv[1]
     def plan_text():
         rows = [("src/app.py", "app()", "trả về 2 thay vì 1", "yêu cầu của bạn"),
                 ("README.md", "cuối file", "ghi một dòng mô tả app()", "tài liệu")]
         lines = ["# Plan: Sửa app() trả về 2", "## Mục tiêu", "Hàm app() trả về 2; README ghi lại thay đổi.",
                  "## Thay đổi", "| # | File | Dòng / hàm | Sửa gì | Lý do |", "|---|---|---|---|---|"]
         for i, (f, w, what, why) in enumerate(rows, 1):
             lines.append(f"| {i} | {f} | {w} | {what} | {why} |")
         lines += ["## Không được đụng tới", "", "## Cách kiểm tra", "python3 -c 'import src.app'",
                   "## Vướng mắc (Dev ghi vào đây nếu có)", "", ""]
         return "\n".join(lines)
     def lead_plan():
         return {"ops": [{"op": "read", "path": ".plan.md"}, {"op": "read", "path": "src/app.py"},
                         {"op": "read", "path": "README.md"},
                         {"op": "write", "path": ".plan.md", "content": plan_text(), "keep_meta": True}]}
     def lead_append():
         return {"ops": [{"op": "write", "path": ".plan.md", "content": "\n<!-- v2 -->\n", "append": True}]}
     def review(verdict, blocking, advisory):
         body = f"---\nverdict: {verdict}\nblocking: {blocking}\nadvisory: {advisory}\nfiles: []\n---\n# Review: sửa app\n## Mục chặn (blocking)\n"
         body += "".join(f"- mục chặn {i + 1}\n" for i in range(blocking))
         body += "## Góp ý (advisory)\n" + "".join(f"- góp ý {i + 1}\n" for i in range(advisory))
         return body
     def plan_review(verdict, blocking, advisory):
         return {"ops": [{"op": "write", "path": ".plan-review.md", "content": review(verdict, blocking, advisory)}]}
     def code_review(verdict, blocking):
         return {"ops": [{"op": "write", "path": ".code-review.md", "content": review(verdict, blocking, 1)}]}
     def dev_impl(slug):
         return {"ops": [{"op": "read", "path": ".plan.md"}, {"op": "read", "path": "src/app.py"},
                         {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
                         {"op": "write", "path": "README.md", "content": "\napp() trả về 2.\n", "append": True},
                         {"op": "git", "args": ["add", "src/app.py", "README.md"]},
                         {"op": "git", "args": ["commit", "-m", f"{slug}: sửa app"]}]}
     def qa(verdict):
         report = ("# QA: sửa app\n## Thay đổi có khớp plan không (từng mục)\n- src/app.py: đã xem.\n- README.md: đã xem.\n"
                   f"## Sửa ngoài plan (nếu có)\nKhông.\n## Rủi ro\nKhông.\n## Kết luận\n{verdict}\n")
         return {"ops": [{"op": "write", "path": ".qa-report.md", "content": report}]}
     sc = {
         "gate": {"by_role": {"lead": [lead_plan()], "plan-review": [plan_review("approve", 0, 2)]}},
         "full": {"by_role": {"lead": [lead_plan()], "plan-review": [plan_review("approve", 0, 1)],
                              "dev": [dev_impl("tele-gate")], "code-review": [code_review("pass", 0)],
                              "qa": [qa("Đạt")]}},
         "plan-cap": {"by_role": {"lead": [lead_plan(), lead_append()],
                                  "plan-review": [plan_review("revise", 1, 0), plan_review("revise", 1, 1),
                                                  plan_review("revise", 1, 2)]}},
         "dev-sleep": {"by_role": {"dev": [{"sleep": 600, "ops": []}]}},
     }
     for name, v in sc.items():
         with open(os.path.join(out, name + ".json"), "w", encoding="utf-8") as f:
             json.dump(v, f, ensure_ascii=False, indent=2)
     print("scenarios:", ", ".join(sorted(sc)))
     PY
     ```
- **Lệnh thực hiện** (kiểm tra môi trường, chưa ghép nối):
  ```bash
  "$AB" status demo-tele                       # đường dẫn nhanh: nhánh main, sạch
  sha256sum "$AB"
  (umask 077; "$AB" serve --port 7439 > "$LIVE/serve.out" 2>&1 < /dev/null & echo $! > "$LIVE/serve.pid")
  SERVE_PID=$(cat "$LIVE/serve.pid"); echo "serve pid $SERVE_PID"
  sed 's/token=[0-9a-f]*/token=<ẩn>/' "$LIVE/serve.out"
  grep -c '^Telegram' "$LIVE/serve.out"        # 0: chưa ghép nối thì không có bot
  "$AB" telegram test; echo "exit=$?"
  kill -INT "$SERVE_PID"; python3 -c 'import time; time.sleep(2)'; ps -p "$SERVE_PID" -o pid,args
  ```
- **Kết quả mong đợi**:
  1. `status` in `Nhánh: main` và `(sạch)`.
  2. `serve.out` có đúng hai dòng `agent-brain dashboard: http://127.0.0.1:7439/?token=…` và `Mở URL trên trong trình duyệt. …`; đếm dòng `Telegram` được `0`.
  3. `telegram test` in `Chưa ghép nối Telegram — chạy agent-brain telegram setup` và `exit=1`.
  4. Sau SIGINT, `serve.out` có `Đã dừng dashboard.` và `ps -p` chỉ in dòng tiêu đề. `serve` cổng 7437 của bạn vẫn chạy (`ps -p <PID trong STATE.md> -o pid,args`), không bị gửi tín hiệu.
- **Ghi nhận**: Điền phần `Môi trường (P5.3-00)` ở đầu `protocol-log/nghiem-thu-p5-3.md`: commit của repo, sha256 của binary, ngày, và kết quả bốn mục trên.
- **Dọn dẹp**: Không. Giữ `$LIVE` cho các ca sau.

---

### P5.3-01: Ghép nối bot và tin thử
- **Mục tiêu**: `agent-brain telegram setup` không hiện token khi dán, ghép nối qua mã 6 số gửi từ chat riêng trên điện thoại, xác nhận ở terminal và trong Telegram, để lại `telegram.json` quyền `600`; `telegram test` tới được điện thoại; `serve` bật bot (TELE-01).
- **Tiền điều kiện**: P5.3-00 xong; không có `serve` nào của `$LIVE` đang chạy; bạn đã tạo bot bằng @BotFather và có token.
- **Lệnh thực hiện**:
  1. **Bạn**, trong terminal của bạn:
     ```bash
     source <đường dẫn $LIVE>/env.sh
     "$AB" telegram setup
     ```
     Dán token sau dòng `Token bot (từ @BotFather): ` rồi Enter: **không** có ký tự nào hiện ra. Setup in `Mở Telegram, nhắn cho @<tên bot> đúng mã sau (hết hạn sau 10 phút): NNNNNN`. Trên điện thoại, mở chat riêng với bot, nhắn đúng 6 số đó.
  2. Kiểm tra (Claude hoặc bạn, cùng `env.sh`; không in nội dung file):
     ```bash
     stat -c %a "$XDG_CONFIG_HOME/agent-brain/telegram.json"
     "$AB" telegram test; echo "exit=$?"
     ```
  3. Bật lại dashboard có bot và ghi PID:
     ```bash
     (umask 077; "$AB" serve --port 7439 > "$LIVE/serve.out" 2>&1 < /dev/null & echo $! > "$LIVE/serve.pid")
     SERVE_PID=$(cat "$LIVE/serve.pid"); echo "serve pid $SERVE_PID"
     python3 -c 'import time; time.sleep(5)'; grep '^Telegram' "$LIVE/serve.out"
     ```
- **Kết quả mong đợi**:
  1. Lúc dán token, terminal không hiện gì (kể cả dấu `*`); sau mã đúng, terminal in `Đã ghép nối với chat … (@<tên bot>). Đã lưu …/telegram.json (quyền 0600). Khởi động lại agent-brain serve để bật thông báo.` và setup thoát `0`.
  2. Điện thoại nhận `Đã ghép nối agent-brain với chat này. Bạn sẽ nhận tin khi task cần bạn duyệt hoặc xử lý.`
  3. `stat` in `600`.
  4. `telegram test` in `Đã gửi tin thử tới chat ….`, `exit=0`; điện thoại nhận `Tin thử từ agent-brain trên máy <tên máy>: kết nối Telegram hoạt động.`
  5. `serve.out` có `Telegram: đã kết nối (@<tên bot>)`.
- **Ghi nhận**: Dòng `P5.3-01`: tên bot (`@…`, không phải token), có / không hiện chữ khi dán, kết quả `stat`, hai tin trên điện thoại, dòng `Telegram: đã kết nối`. Chat: `đã ghép (ẩn)`.
- **Dọn dẹp**: Không. Giữ `serve` (PID `$SERVE_PID`) cho các ca sau.

---

### P5.3-02: Tin chờ duyệt plan kèm file plan
- **Mục tiêu**: Một task chạy bằng `agent-brain auto` trong terminal dừng ở cổng duyệt; điện thoại nhận file `{slug}.plan.md` (mở và đọc được đủ) và tin chờ duyệt với các trường của D-11 (TELE-04, TELE-05).
- **Tiền điều kiện**: P5.3-01 xong, `serve` đang chạy; `demo-tele` ở `main`, sạch.
- **Lệnh thực hiện**:
  ```bash
  use_sc gate
  "$AB" auto --start --name tele-gate -- demo-tele "Sửa app() trả về 2"; echo "exit=$?"
  q "SELECT id, status, auto_state, approved_commit FROM tasks"
  grep 'Telegram: đã gửi' "$LIVE/serve.out"
  ```
  Trên điện thoại: mở file `tele-gate.plan.md` rồi đọc tin.
- **Kết quả mong đợi**:
  1. Terminal: `Autopilot dừng ở bước duyệt plan (task 1) — duyệt: agent-brain approve 1`, `exit=0`; SQL: `1 | planned | gate | None`.
  2. Điện thoại nhận file `tele-gate.plan.md` (nội dung đủ: `# Plan: Sửa app() trả về 2`, bảng `Thay đổi` hai dòng), ngay sau đó là tin:
     `Chờ duyệt plan · task 1` / tiêu đề task / `Dự án: demo-tele · profile work · nhánh task/tele-gate` / `Plan: Sửa app() trả về 2` / `Thay đổi (2 file):` với `- src/app.py`, `- README.md` / `Review plan: duyệt · vòng 1 / 3`.
  3. Nút: `Duyệt plan`, `Trả về Lead` (không có `Kill`: không có run nào đang chạy).
  4. `serve.out`: đúng một dòng `Telegram: đã gửi tin chờ duyệt plan cho task 1`.
- **Ghi nhận**: Dòng `P5.3-02`: nguyên văn tin (không có ID chat), tên file, nhãn các nút, dòng `serve.out`.
- **Dọn dẹp**: Không. Task 1 dùng tiếp ở P5.3-03.

---

### P5.3-03: Duyệt plan từ điện thoại tới QA đạt
- **Mục tiêu**: Nút `Duyệt plan` trên điện thoại duyệt đúng commit plan, autopilot chạy tiếp tới QA đạt, và điện thoại nhận tin `QA đạt` (TELE-06, TELE-04).
- **Tiền điều kiện**: P5.3-02 xong (task 1 chờ duyệt, `demo-tele` đang ở `task/tele-gate`).
- **Lệnh thực hiện**:
  1. Đổi kịch bản cho các bước sau cổng (Dev, Review code, QA):
     ```bash
     use_sc full
     ```
  2. Trên điện thoại: bấm `Duyệt plan` trên tin của task 1.
  3. Chờ khoảng 10 giây, rồi:
     ```bash
     q "SELECT id, status, auto_state, approved_commit FROM tasks WHERE id = 1"
     q "SELECT id, role, stage, round, status, outcome FROM runs WHERE task_id = 1 ORDER BY id"
     grep 'Telegram:' "$LIVE/serve.out"
     ```
- **Kết quả mong đợi**:
  1. Tin được sửa thành `… ✓ Đã duyệt plan <sha7> · autopilot chạy tiếp — từ Telegram lúc HH:MM` và mất nút; `serve.out` có `Telegram: Duyệt plan task 1 — …`.
  2. Run: `lead · plan`, `plan-review · review:approve`, `dev · implement`, `code-review · review:pass`, `qa · qa:pass`, tất cả `succeeded`; task `qa_pass | done | <sha đầy đủ của commit plan>` (`<sha7>` của tin là 7 ký tự đầu).
  3. Điện thoại nhận `QA đạt · task 1` / tiêu đề / dòng dự án / `Tổng: 5 run · … phút · $…`, không có nút; `serve.out`: `Telegram: đã gửi tin QA đạt cho task 1`.
- **Ghi nhận**: Dòng `P5.3-03`: tin đã sửa, kết quả hai câu SQL (chép nguyên văn), tin QA đạt.
- **Dọn dẹp**: `git -C "$LIVE/repos/demo-tele" checkout main` (phải sạch: `git -C "$LIVE/repos/demo-tele" status --short` không in gì).

---

### P5.3-04: Các nút ở lần dừng: Trả về Lead có ghi chú, Chạy thêm 1 vòng, Dừng hẳn
- **Mục tiêu**: Tin `Autopilot dừng` có lý do và các nút của D-12; `Trả về Lead` hỏi ghi chú và run Lead sau nhận đúng ghi chú đó; `Chạy thêm 1 vòng` và `Dừng hẳn` làm đúng như trên dashboard (TELE-05, TELE-06).
- **Tiền điều kiện**: P5.3-03 xong; `demo-tele` ở `main`, sạch.
- **Lệnh thực hiện**:
  1. Tạo một task luôn bị Review plan yêu cầu sửa:
     ```bash
     use_sc plan-cap
     "$AB" auto --start --name tele-stop -- demo-tele "Sửa app() trả về 2 (dừng hết vòng)"; echo "exit=$?"
     q "SELECT id, auto_state, json_extract(auto_json, '\$.stop.reason') FROM tasks WHERE id = 2"
     ```
  2. Trên điện thoại, tin `Autopilot dừng · hết vòng · task 2`: bấm `Trả về Lead`. Bot hỏi ghi chú; nhắn một câu ngắn, ví dụ `Thêm test cho app()`. Chờ tới tin dừng mới của task 2, rồi:
     ```bash
     q "SELECT id, role, round, status, json_extract(metrics_json, '\$.note') FROM runs WHERE task_id = 2 ORDER BY id"
     grep -c 'Thêm test cho app()' "$FAKE_CLAUDE_RECORD"      # ghi chú có trong lời nhắn gửi cho Lead
     ```
  3. Trên tin dừng mới: bấm `Chạy thêm 1 vòng`. Chờ tới tin dừng tiếp theo.
  4. Trên tin dừng đó: bấm `Dừng hẳn`. Rồi:
     ```bash
     q "SELECT id, auto_state, json_extract(auto_json, '\$.stop.reason') FROM tasks WHERE id = 2"
     grep 'Telegram:' "$LIVE/serve.out" | tail -8
     ```
- **Kết quả mong đợi**:
  1. Bước 1: `Autopilot dừng · hết vòng: Review plan vẫn yêu cầu sửa lại sau 3 / 3 vòng. (task 2)`, `exit=4`; SQL `2 | stopped | het_vong`. Điện thoại nhận `Autopilot dừng · hết vòng · task 2` / tiêu đề / dòng dự án / chi tiết lý do, với các nút `Chạy thêm 1 vòng`, `Trả về Lead`, `Dừng hẳn`.
  2. Bước 2: bot gửi `Ghi chú cho Lead (task 2): gửi tin nhắn kế tiếp làm ghi chú (tối đa 500 ký tự), hoặc bấm Không ghi chú. Hết hạn sau 10 phút.` với `Không ghi chú` / `Huỷ`; sau tin nhắn của bạn, lời hỏi đổi thành `Đã gửi cho Lead.`, tin dừng được sửa `✓ Đã trả về Lead (ghi chú: «Thêm test cho app()») — từ Telegram lúc HH:MM`. Autopilot chạy Lead vòng 4 rồi Review plan vòng 4 và dừng lại: tin mới `Autopilot dừng · hết vòng · task 2` với chi tiết `… sau 4 / 3 vòng.` SQL: run Lead vòng 4 có ghi chú `Thêm test cho app()` (các run Lead trước: `None`); `grep -c` lớn hơn hoặc bằng `1`.
  3. Bước 3: tin được sửa `✓ Đã giao thêm 1 vòng — từ Telegram lúc HH:MM`; thêm một vòng plan ⇄ review rồi một tin dừng mới.
  4. Bước 4: tin được sửa `✓ Đã dừng hẳn autopilot. Bật autopilot trên dashboard để chạy tiếp. — từ Telegram lúc HH:MM`; SQL `2 | ended | …`. Ghi lại xem sau đó có tin `Chờ duyệt plan · task 2` không (05.3-03 dự kiến có, vì plan vẫn chờ duyệt; **không** bấm nút của tin này).
  5. `serve.out` có một dòng `Telegram: {nút} task 2 — …` cho mỗi lần bấm.
- **Ghi nhận**: Dòng `P5.3-04`: các tin (nguyên văn, không ID chat), kết quả SQL ở bước 2 và 4, có / không có tin chờ duyệt sau Dừng hẳn.
- **Dọn dẹp**: `git -C "$LIVE/repos/demo-tele" checkout -f main` (task 2 để nguyên; nhánh được xóa cùng `$LIVE`).

---

### P5.3-05: Kill có xác nhận
- **Mục tiêu**: Khi task có run đang chạy, tin của nó có thêm nút `Kill`; bấm `Kill` chỉ hỏi `Xác nhận Kill run {id}?`; `Huỷ` giữ run chạy; xác nhận thì run dừng và cả cây tiến trình biến mất (TELE-06).
- **Tiền điều kiện**: P5.3-04 xong; `demo-tele` ở `main`, sạch.
- **Lệnh thực hiện**:
  1. Một task dừng ở `hết vòng`, rồi Chạy Dev từ dashboard với Dev ngủ 600 giây:
     ```bash
     use_sc plan-cap
     "$AB" auto --start --name tele-kill -- demo-tele "Sửa app() trả về 2 (thử Kill)"; echo "exit=$?"
     ID5=$(q "SELECT id FROM tasks WHERE branch = 'task/tele-kill'"); echo "task $ID5"
     use_sc dev-sleep
     dash_post "/api/tasks/$ID5/dev"            # hoặc bấm Chạy Dev ở dashboard 7439 (URL trong serve.out)
     python3 -c 'import time; time.sleep(3)'
     q "SELECT id, pid, pgid FROM runs WHERE task_id = ? AND status = 'running'" "$ID5"
     ps -eo pid,ppid,pgid,args | awk -v p="$SERVE_PID" '$2 == p'      # CHILD: agent-brain run dev -- demo-tele
     ```
     Ghi lại `RUN` (id run), `CLAUDE` (`pid` của run, fake claude) và `CHILD` (tiến trình `agent-brain run dev` con của serve).
  2. Trên điện thoại, tin `Autopilot dừng · hết vòng · task N` của task này: nút được làm mới, có `Kill` ở cuối (cùng `Chạy thêm 1 vòng`, `Dừng hẳn`; không có `Trả về Lead` khi run đang chạy). Bấm `Kill`.
  3. Bấm `Huỷ`. Chờ 5 giây:
     ```bash
     ps -p "<CHILD>,<CLAUDE>" -o pid,args
     q "SELECT id, status FROM runs WHERE id = ?" "<RUN>"
     ```
  4. Bấm `Kill`, rồi bấm `Kill` lần nữa trên lời xác nhận. Chờ 5 giây:
     ```bash
     ps -p "<CHILD>,<CLAUDE>" -o pid,args
     ps -eo pid,ppid,pgid,args | awk -v g="<CLAUDE>" '$3 == g'
     q "SELECT id, status FROM runs WHERE id = ?" "<RUN>"
     grep 'Telegram: Kill' "$LIVE/serve.out"
     ```
- **Kết quả mong đợi**:
  1. `dash_post` trả `{"job_id":…}`; có đúng một run Dev `running`.
  2. Sau lần bấm `Kill` đầu: tin được sửa, thêm dòng `Xác nhận Kill run R?` với hai nút `Kill` / `Huỷ`; run vẫn `running`.
  3. Sau `Huỷ`: thông báo `Đã huỷ Kill`, dòng xác nhận biến mất; `ps -p` vẫn in cả `CHILD` và `CLAUDE`; run vẫn `running`.
  4. Sau xác nhận: tin được sửa `✓ Đã dừng run R … — từ Telegram lúc HH:MM`, không còn dòng xác nhận và không còn `Kill`; `ps -p` và câu `awk` chỉ in dòng tiêu đề / không in gì; run `killed`; `serve.out` có `Telegram: Kill task N — Đã dừng run R …`.
- **Ghi nhận**: Dòng `P5.3-05`: RUN, các PID đã ghi, kết quả `ps -p` sau Huỷ và sau xác nhận, trạng thái run, nhãn nút ở từng bước.
- **Dọn dẹp**: `git -C "$LIVE/repos/demo-tele" checkout -f main`.

---

### P5.3-06: Chat lạ bị bỏ qua
- **Mục tiêu**: Lệnh gửi cho bot từ một chat khác chat đã ghép nối (ở đây: nhóm có bạn và bot) không được trả lời và không làm gì; `serve` in một dòng `Telegram: bỏ qua tin từ chat lạ {id}` (TELE-02). Nút bấm của người lạ được test tự động `stranger_chat_ignored` kiểm tra.
- **Tiền điều kiện**: `serve` đang chạy với bot; một nhóm Telegram có bot.
- **Lệnh thực hiện**:
  1. `q "SELECT COUNT(*) FROM tasks"; q "SELECT COUNT(*) FROM runs"` (ghi lại hai số).
  2. Trong nhóm, gửi `/start@<tên bot>` và thêm một lệnh gửi cho bot, ví dụ `/help@<tên bot>` (với chế độ riêng tư mặc định, trong nhóm bot chỉ nhận lệnh gửi cho nó).
  3. Chờ 30 giây, rồi:
     ```bash
     grep -c 'Telegram: bỏ qua tin từ chat lạ' "$LIVE/serve.out"
     q "SELECT COUNT(*) FROM tasks"; q "SELECT COUNT(*) FROM runs"
     ```
- **Kết quả mong đợi**: Bot không trả lời trong nhóm; đếm được đúng `1` dòng `bỏ qua tin từ chat lạ` (ID là ID âm của nhóm; giới hạn một dòng mỗi 10 phút cho mỗi ID); số task và run không đổi.
- **Ghi nhận**: Dòng `P5.3-06`: số dòng, ID ghi `nhóm (ẩn)`, số task / run trước và sau.
- **Dọn dẹp**: Không.

---

### P5.3-07: Khởi động lại serve không gửi lại tin cũ; token không nằm trên dòng lệnh
- **Mục tiêu**: Sau khi khởi động lại `serve`, không tin cũ nào được gửi lại (TELE-04); trong lúc `serve` chạy, `ps -eo args` thấy curl với `-K -` và không thấy token.
- **Tiền điều kiện**: `serve` đang chạy với bot; các task 1–3 ở trạng thái của các ca trước (QA đạt, dừng / chờ duyệt).
- **Lệnh thực hiện**:
  1. Token không có trên dòng lệnh (lúc `serve` đang long poll):
     ```bash
     ps -eo pid,ppid,args | grep '[c]url'
     ps -eo args | grep '[c]url' | grep -cE '[0-9]{8,10}:[A-Za-z0-9_-]{30,}'
     ```
  2. Dừng rồi chạy lại `serve` (không bấm nút nào trong lúc `serve` tắt):
     ```bash
     kill -INT "$SERVE_PID"; python3 -c 'import time; time.sleep(3)'; ps -p "$SERVE_PID" -o pid,args
     (umask 077; "$AB" serve --port 7439 > "$LIVE/serve.out" 2>&1 < /dev/null & echo $! > "$LIVE/serve.pid")
     SERVE_PID=$(cat "$LIVE/serve.pid"); echo "serve pid $SERVE_PID"
     python3 -c 'import time; time.sleep(90)'      # hơn một lần quét an toàn (60 giây)
     grep '^Telegram' "$LIVE/serve.out"
     ```
- **Kết quả mong đợi**:
  1. Có một dòng curl dạng `curl -q -sS --max-time N --proto =https -K -`, PPID là `serve`; câu đếm in `0`.
  2. `ps -p` sau SIGINT chỉ in dòng tiêu đề. `serve.out` mới chỉ có `Telegram: đã kết nối (@<tên bot>)`, không có dòng `Telegram: đã gửi tin`; điện thoại không nhận tin nào trong 90 giây.
- **Ghi nhận**: Dòng `P5.3-07`: dòng curl (không có token), số đếm, các dòng `Telegram` sau khởi động lại, xác nhận của bạn rằng không có tin cũ.
- **Dọn dẹp**: Không. Ghi lại `SERVE_PID` mới.

---

### P5.3-08 (không bắt buộc): Mất mạng một lúc
- **Mục tiêu**: Mất mạng khoảng một phút chỉ in một dòng cảnh báo; tin phát sinh trong lúc mất mạng được gửi sau khi có mạng lại; agent vẫn chạy (D-05).
- **Tiền điều kiện**: `serve` đang chạy với bot; tắt mạng được mà không ảnh hưởng việc khác của bạn.
- **Lệnh thực hiện**:
  1. Tắt mạng của máy (Wi-Fi / cáp). Chờ khoảng 30 giây.
  2. Trong lúc mất mạng, tạo một task dừng ở cổng trên repo `nhanh`:
     ```bash
     use_sc gate
     "$AB" run lead --name tele-outage -- demo-tele-nhanh "Sửa app() trả về 2 (lúc mất mạng)"; echo "exit=$?"
     ```
  3. Chờ thêm khoảng 30 giây, bật mạng lại, chờ tới 60 giây, rồi `grep '^Telegram' "$LIVE/serve.out" | tail -5`.
- **Kết quả mong đợi**: `run lead` thoát `0` dù mất mạng; `serve.out` có đúng một dòng `Telegram: không tới được Telegram (…) — đang thử lại; …` và sau đó một dòng `Telegram: đã kết nối lại`; điện thoại nhận file `tele-outage.plan.md` và tin `Chờ duyệt plan · task N` (`Review plan: không có trong quy trình nhanh`) đúng một lần sau khi có mạng lại.
- **Ghi nhận**: Dòng `P5.3-08`: các dòng `Telegram`, thời điểm tin tới; hoặc `không chạy` kèm lý do.
- **Dọn dẹp**: Bật lại mạng nếu chưa bật.

---

### Dọn dẹp
1. Dừng `serve` của đợt nghiệm thu bằng PID đã ghi và kiểm tra:
   ```bash
   kill -INT "$SERVE_PID"; python3 -c 'import time; time.sleep(3)'
   ps -p "$SERVE_PID" -o pid,args                 # chỉ dòng tiêu đề
   q "SELECT COUNT(*) FROM runs WHERE status = 'running'"   # 0
   ```
   `ps -p` với mọi PID đã ghi ở P5.3-05 cũng chỉ in dòng tiêu đề.
2. Ghi xong `protocol-log/nghiem-thu-p5-3.md`, chạy kiểm tra dạng token trên file đó (không có dòng nào dạng token bot), rồi xóa toàn bộ môi trường: `rm -rf "$LIVE"` (gồm `telegram.json` có token, `brain.db`, hai repo nháp và log run).
3. Không bắt buộc: thu hồi token của bot thử trong @BotFather (`/revoke`), hoặc giữ bot và sau này chạy `agent-brain telegram setup` (không đặt `XDG_CONFIG_HOME`) để ghép nối cấu hình thật của bạn. Khi đó, tắt `serve` thật trước (setup từ chối khi `serve` đang chạy trên cùng thư mục dữ liệu) và chạy lại nó sau.
4. Không lệnh nào trong phần này ghi vào `~/.local/share/agent-brain`, `~/.config/agent-brain` hay `~/.claude-orca`, và `serve` cổng 7437 của bạn không bị gửi tín hiệu.
