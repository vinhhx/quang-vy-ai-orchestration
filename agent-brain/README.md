# agent-brain — Bộ công cụ điều phối giao thức lõi bằng Rust

`agent-brain` là binary dòng lệnh độc lập, tĩnh hoàn toàn (statically linked musl), được xây dựng ở Phase 3 của dự án QuangVy-Orchestration nhằm thay thế bộ công cụ mẫu Python `brain` (Phase 2, D-02). Binary cung cấp khả năng điều phối 3 vai trò (Lead → Dev → QA), quản lý cây tiến trình an toàn, theo dõi sự kiện trực tiếp và lưu trữ nhật ký vào SQLite nội bộ mà không đòi hỏi thêm bất kỳ runtime hay thư viện ngoài nào trên máy đích.

---

## 1. Quan hệ chuyển giao với Python `brain` (D-02)

- **Chia sẻ danh mục**: `agent-brain` đọc và ghi cùng file danh mục dự án `~/.config/agent-brain/projects_map.json` với cùng định dạng (JSON 2-space, UTF-8 không thoát unicode, giữ nguyên thứ tự khóa).
- **Kế thừa nhánh task**: Các nhánh task `task/<slug>` và commit plan do Python `brain` tạo ra trước đây được `agent-brain` nhận diện và tiếp tục điều phối liền mạch.
- **Tách biệt dữ liệu**: `agent-brain` ghi nhật ký trạng thái và số liệu token vào database SQLite cục bộ `~/.local/share/agent-brain/brain.db` thay vì chỉ ghi file text.
- **Chuyển giao hoàn tất**: Sau khi nghiệm thu đạt, người dùng gỡ symlink Python `brain` bằng `bash brain/install.sh --uninstall` và sử dụng duy nhất binary `agent-brain`.

---

## 2. Biên dịch & Cài đặt

### Biên dịch tĩnh (musl)
Yêu cầu môi trường đã cài target `x86_64-unknown-linux-musl` và công cụ biên dịch C `musl-gcc` (Route A). Lệnh biên dịch hoàn toàn ngoại tuyến và cố định phiên bản (`--offline --locked`):
```bash
bash scripts/build-release.sh
```
Nếu máy thiếu `musl-gcc`, script tự động chuyển sang Route B (`gcc` kèm cờ CFLAGS tắt fortify và LFS cho SQLite bundled) và in cảnh báo.

### Cài đặt vào PATH
Cài binary vào thư mục cá nhân `~/.local/bin` (đảm bảo thư mục này có trong `$PATH`):
```bash
install -m 755 agent-brain/target/x86_64-unknown-linux-musl/release/agent-brain ~/.local/bin/agent-brain
```

Kiểm tra sau cài đặt:
```bash
agent-brain --version
```

---

## 3. Bảng lệnh CLI

| Lệnh | Cú pháp | Ý nghĩa & Hành vi |
|---|---|---|
| **projects scan** | `agent-brain projects scan [--root DIR]...` | Quét các thư mục gốc ở độ sâu 1 để tự động nhận diện repo git và cập nhật `projects_map.json`. Giữ nguyên cấu hình sửa tay cũ. |
| **projects add** | `agent-brain projects add PATH [--type code\|media]` | Đăng ký thủ công một dự án mới vào danh mục với loại `code` (mặc định) hoặc `media`. |
| **run lead** | `agent-brain run lead PROJECT REQUEST [--name SLUG] [--dry-run] [--verbose]` | Khảo sát yêu cầu, tạo nhánh `task/<slug>` và sinh file kế hoạch `.plan.md`. Bị giới hạn chỉ đọc/ghi `.plan.md`. |
| **run dev** | `agent-brain run dev PROJECT [--dry-run] [--verbose]` | Thực thi các chỉnh sửa mã nguồn được liệt kê trong bảng `Thay đổi` của `.plan.md`. Được bảo vệ bởi guard hook. |
| **run qa** | `agent-brain run qa PROJECT [--dry-run] [--verbose]` | Chạy kiểm tra chất lượng trên `git diff` giữa commit plan và HEAD; sinh file `.qa-report.md`. Không sửa code. |
| **status** | `agent-brain status PROJECT` | Đường dẫn nhanh (fast path): hiển thị trạng thái git, nhánh hiện tại, log gần nhất. Tiêu tốn 0 token, không gọi Claude CLI. |
| **inspect** | `agent-brain inspect PROJECT [--verbose]` | Đường dẫn nhanh (fast path): gọi Claude đọc diff chưa commit trong đúng 1 turn (`--max-turns 1`), tự xóa phiên tạm, không lưu session. |
| **kill** | `agent-brain kill TASK_ID` | Dừng an toàn một task đang chạy theo `TASK_ID` (dựa trên PID và thời điểm khởi tạo trong DB), gửi SIGINT và dọn sạch cây tiến trình con. |
| **stats** | `agent-brain stats [--profile P] [--since Nm\|Nh\|Nd] [--json]` | Báo cáo thống kê lượt chạy, thời gian, số token tiêu thụ và chi phí tương đương từ SQLite. Hỗ trợ lọc theo profile và thời gian. |
| **guard** | `agent-brain guard --policy FILE` | Hook nội bộ do Claude CLI gọi qua `--settings`: kiểm tra quyền đọc/ghi từng file theo policy allowlist của plan. |
| **serve** | `agent-brain serve [--port N]` | Mở dashboard web trong chính binary, chỉ 127.0.0.1, có token. Cổng mặc định 7437, `--port 0` để hệ điều hành tự chọn. |

---

## 4. Bảng mã thoát (Exit Codes)

| Mã thoát | Tên quy ước | Ý nghĩa chi tiết |
|---|---|---|
| `0` | EXIT_OK | Thành công hoàn toàn (hoặc QA kết luận Đạt/Cần sửa hợp lệ). |
| `1` | EXIT_ERROR | Lỗi thực thi, cấu hình không hợp lệ, thiếu file, vi phạm tiền điều kiện git. |
| `2` | EXIT_ROUTING | Fallback Ask: Không khớp hoặc khớp nhiều hơn 1 dự án (in danh sách để người dùng chọn). |
| `3` | EXIT_PROTOCOL | Vi phạm giao thức: Sửa file ngoài danh sách cho phép của plan (ROLE-01, ROLE-06). |
| `4` | EXIT_BLOCKED | Vướng mắc: Dev gặp trở ngại hoặc thiếu thông tin, ghi mục Vướng mắc vào `.plan.md` và dừng lại an toàn. |
| `130` | EXIT_INTERRUPTED | Dừng bởi người dùng hoặc hệ thống: nhận tín hiệu bàn phím (Ctrl+C) hoặc bị dừng qua lệnh `agent-brain kill`. |

---

## 5. Cấu hình & Biến môi trường

### File cấu hình
- **Đường dẫn**: Mặc định đọc tại `~/.config/agent-brain/config.json`. Nếu file chưa tồn tại, hệ thống tự động sử dụng cấu hình mặc định nhúng sẵn trong binary mà không báo lỗi.
- **Biến ghi đè**: Đặt `AGENT_BRAIN_CONFIG=/duong/dan/config.json` để chỉ định file cấu hình khác.

### File cơ sở dữ liệu
- **Đường dẫn**: Mặc định lưu tại `~/.local/share/agent-brain/brain.db`.
- **Biến ghi đè**: Đặt `AGENT_BRAIN_DB=/duong/dan/brain.db` để chuyển hướng DB (rất hữu ích cho môi trường kiểm thử cách ly).

### Các khóa cấu hình và giá trị mặc định

```json
{
  "claude_bin": "claude",
  "models": {
    "lead": "opus",
    "dev": "sonnet",
    "qa": "sonnet"
  },
  "inspect_model": "sonnet",
  "profiles": {
    "work": {
      "claude_config_dir": null,
      "unset_env": [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL"
      ]
    },
    "personal": {
      "claude_config_dir": "~/.claude-orca",
      "unset_env": [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL"
      ]
    }
  },
  "common_args": [
    "--strict-mcp-config"
  ],
  "max_turns": {
    "lead": null,
    "dev": null,
    "qa": null
  },
  "timeout_seconds": {
    "lead": 1800,
    "dev": 1800,
    "qa": 1200,
    "inspect": 300
  },
  "runs_dir": null,
  "kill_grace_secs": 3,
  "inspect_max_diff_bytes": 100000,
  "dashboard_port": 7437
}
```

- `claude_bin`: Đường dẫn thực thi CLI agent (mặc định `"claude"`).
- `models`: Model chỉ định theo vai trò (lead = opus, dev = sonnet, qa = sonnet).
- `inspect_model`: Model phục vụ lệnh `inspect` (mặc định `"sonnet"`).
- `profiles`: Cấu hình thư mục cấu hình và các biến cần xóa cho từng tài khoản.
- `common_args`: Danh sách đối số chung cho Claude CLI (mặc định `["--strict-mcp-config"]`).
- `max_turns`: Giới hạn số lượt trao đổi (null nghĩa là không giới hạn).
- `timeout_seconds`: Thời gian chạy tối đa (giây) cho từng vai trò.
- `runs_dir`: Thư mục lưu log chi tiết từng run (mặc định `~/.local/share/agent-brain/runs/`).
- `kill_grace_secs`: Thời gian chờ (mặc định `3` giây) giữa tín hiệu SIGINT ban đầu và tín hiệu SIGKILL cưỡng chế khi dừng tiến trình.
- `inspect_max_diff_bytes`: Giới hạn dung lượng diff tối đa (100.000 byte) gửi cho Claude khi `inspect` (tránh tràn token).
- `dashboard_port`: Cổng lắng nghe mặc định của dashboard web (mặc định `7437`). Thứ tự ưu tiên: cờ `--port` trên dòng lệnh > `dashboard_port` trong file cấu hình > cổng mặc định 7437; truyền `--port 0` để hệ điều hành tự chọn một cổng ngẫu nhiên chưa sử dụng.

---

## 6. Hồ sơ tài khoản (Profiles) & Ràng buộc an toàn

- **`work`**: Dành cho công việc, sử dụng cấu hình mặc định của Claude Code tại `~/.claude` (biến `CLAUDE_CONFIG_DIR` được bỏ đặt).
- **`personal`**: Dành cho dự án cá nhân, trỏ tới `~/.claude-orca` (`CLAUDE_CONFIG_DIR=~/.claude-orca`).
- **Từ chối profile rỗng**: Nếu dự án trong `projects_map.json` chưa được gán profile (`profile: ""`), `agent-brain` từ chối chạy agent và yêu cầu người dùng cấu hình rõ ràng.
- **Cấm chạy song song trên `personal` (E-06)**: Vì `~/.claude-orca` có thể dùng chung với phiên làm việc chính của người dùng, không bao giờ khởi chạy hai tác vụ cùng lúc trên profile `personal` để tránh hiện tượng xung đột xoay vòng token chứng thực.

---

## 7. Định danh Task, Run & Nhánh Git

- **Cặp định danh**: Mỗi lần thực thi được gán một cặp `task N, run M` duy nhất trong SQLite:
  - `task N`: Tương ứng với một nhánh `task/<slug>` cụ thể trên repo dự án.
  - `run M`: Định danh lần chạy thứ `M` của task đó (ví dụ run 1 là Lead tạo plan, run 2 là Dev code, run 3 là QA kiểm tra).
- **Kế thừa nhánh**: Khi chạy `run dev` hoặc `run qa`, `agent-brain` tự động xác định nhánh task hiện tại và liên kết đúng `task_id` từ lịch sử trong cơ sở dữ liệu.

---

## 8. Cơ chế dừng tiến trình (Kill & Signal Handling)

- **Process Group Isolation**: Mọi tiến trình con Claude được khởi chạy trong một Process Group riêng biệt (`setpgid`).
- **Lệnh `agent-brain kill <task_id>`**:
  1. Đọc PID và `start_time` của task từ SQLite.
  2. Đối chiếu với `/proc/<pid>/stat` để đảm bảo hệ điều hành chưa tái sử dụng PID cho tiến trình khác. Tuyệt đối không gửi tín hiệu nếu PID đã bị tái sử dụng.
  3. Gửi SIGINT tới toàn bộ process group.
  4. Chờ tối đa `kill_grace_secs` (mặc định 3 giây). Nếu tiến trình hoặc các nhánh con (`sleep`, `bash`) chưa dừng hẳn, gửi tiếp SIGKILL dọn sạch.
  5. Cập nhật trạng thái run trong DB thành `killed`.
- **Tín hiệu bàn phím (Ctrl+C)**:
  - Nhấn Ctrl+C một lần: Bắt đầu quy trình dừng mềm (graceful shutdown), gửi tín hiệu cho con và chờ dọn dẹp.
  - Nhấn Ctrl+C hai lần nhanh: Kích hoạt ngắt khẩn cấp ngay lập tức (< 1 giây).
- **Đóng cửa sổ terminal (SIGHUP)**: Tự động truyền tín hiệu dừng tới toàn bộ nhóm tiến trình con; không để sót tiến trình mồ côi.
- **Trạng thái kết thúc**: Các trạng thái dừng gồm `killed` (bị ngắt có chủ đích), `timed_out` (quá thời gian timeout cấu hình), hoặc `lost` (phát hiện tiến trình đã biến mất bất thường khi reconcile).

---

## 9. Đường dẫn nhanh (Fast Paths)

- **`status`**: Không gọi Claude CLI, không tiêu tốn token (0 token). Chỉ truy vấn `git status`, `git branch`, diffstat và đọc nhanh bản ghi run mới nhất trong DB.
- **`inspect`**:
  - Dành cho việc review nhanh diff chưa commit trước khi quyết định chạy tiếp.
  - Gọi Claude CLI với `--max-turns 1`, `--tools ""` và `--no-session-persistence`.
  - Giới hạn diff ở mức `inspect_max_diff_bytes`. Nếu vượt quá, thông báo cắt gọn an toàn.
  - Sau khi nhận nhận xét, tự động dọn dẹp thư mục phiên tạm thời nếu CLI có tạo ra file nháp.

---

## 10. Thống kê & Chi phí tương đương (Stats)

- Lệnh `agent-brain stats` tổng hợp toàn bộ lịch sử từ `brain.db`.
- **Bộ lọc**: Hỗ trợ `--profile work|personal` và `--since Nm|Nh|Nd` (ví dụ `--since 7d` cho 7 ngày gần nhất).
- **Xuất JSON**: Cờ `--json` xuất toàn bộ số liệu ra định dạng có cấu trúc chứa trường `total`, `by_project`, `by_role`.
- **Chi phí tương đương**: Tính toán ước tính chi phí USD dựa trên bảng giá chuẩn cho input tokens, output tokens, cache read và cache creation của từng model đã dùng.

---

## 11. An toàn dữ liệu & Phân quyền file

- File cơ sở dữ liệu `brain.db` được tạo với quyền `0600` (chỉ người dùng hiện tại có quyền đọc/ghi).
- Các thư mục lưu trữ (`runs/`, thư mục DB) được tạo với quyền `0700`.
- Mọi dòng sự kiện stream-json thô từ Claude CLI được lưu nguyên vẹn vào cột `raw_json` của bảng `events`, cho phép kiểm toán độc lập bất kỳ lúc nào mà không sợ parse sai lệch.

---

## 12. Kiểm thử phần mềm

Tất cả các lệnh kiểm thử đều chạy offline và khóa phiên bản:
```bash
# Toàn bộ test suite của Rust (chạy đơn luồng để tránh xung đột file tạm)
cargo test --manifest-path agent-brain/Cargo.toml --offline --locked -- --test-threads=1

# Bộ kiểm thử hợp đồng và hồi quy Python
python3 -m unittest discover -s tests -t . -q
```

Bao gồm các bài kiểm thử tự động cho lõi điều phối và dashboard:
- `tests/dashboard_http.rs`: Kiểm tra handshake xác thực bằng token trên query param `/?token=TOKEN` (chuyển hướng 303 và cấp cookie), kiểm tra từ chối 401 khi thiếu cookie, chặn Host header lạ (421 Misdirected Request) và các route API JSON.
- `tests/dashboard_assets.rs`: Kiểm tra các file tĩnh nhúng trong binary (HTML, CSS Tailwind đã build, JS Alpine) tải thành công với Content-Type phù hợp và trả về 404 cho đường dẫn không tồn tại.
- `tests/dashboard_run.rs`, `tests/dashboard_pipeline.rs`: Kiểm tra vòng đời chạy và chuyển trạng thái của task Lead → Dev → QA từ dashboard.
- `tests/dashboard_stream.rs`: Kiểm tra kênh truyền log sự kiện theo thời gian thực qua Server-Sent Events (SSE).
- `tests/dashboard_kill.rs`: Kiểm tra endpoint `/api/runs/:id/kill` dừng an toàn tiến trình và dọn dẹp cây tiến trình.
- `tests/dashboard_projects.rs`: Kiểm tra API danh sách dự án, cập nhật alias, quét repo và phát hiện tên trùng lặp.
- `tests/dashboard_stats.rs`: Kiểm tra báo cáo thống kê token theo ngày, vai trò và profile qua API.

---

## 13. Giả định CLI Claude Code mở

- **Giả định A1 (`--max-turns`)**: Cờ `--max-turns` không xuất hiện trong `claude --help` phiên bản 2.1.301 nhưng vẫn được CLI chấp nhận và xử lý giới hạn lượt ở chế độ non-interactive. Giả định này được đưa vào kiểm chứng thực tế trong tài liệu nghiệm thu `LIVE-CHECKS.md`.

---

## 14. Dashboard web (agent-brain serve)

Lệnh `agent-brain serve [--port N]` khởi động giao diện web nội bộ được nhúng sẵn hoàn toàn bên trong binary, phục vụ việc theo dõi thời gian thực, giao việc, xem tiến trình pipeline và thống kê chi phí mà không cần cài đặt Node.js hay bất kỳ máy chủ ngoài nào.

### Khởi động & Cơ chế chứng thực (Token & Cookie)
- **Khởi động**: Chạy lệnh trên terminal:
  ```bash
  agent-brain serve
  ```
  Binary sẽ in URL chứa token truy cập một lần:
  ```text
  agent-brain dashboard: http://127.0.0.1:7437/?token=TOKEN
  Mở URL trên trong trình duyệt. Nhấn Ctrl+C để dừng dashboard (các run do dashboard khởi chạy cũng dừng theo).
  ```
- **Mở đúng máy chủ `127.0.0.1`**: Luôn mở địa chỉ `http://127.0.0.1:PORT/?token=TOKEN` (cookie gắn với host cụ thể; không dùng `localhost`).
- **Cơ chế token & session cookie**: Token ngẫu nhiên (32 byte hex = 64 ký tự từ `/dev/urandom`) được sinh mới mỗi lần chạy `serve` và **không bao giờ ghi xuống đĩa**. Khi trình duyệt gửi yêu cầu `GET /?token=TOKEN`, server xác thực và phản hồi `303 See Other` về `/` kèm cookie phiên `agent_brain_token` với cờ `HttpOnly; SameSite=Strict; Path=/`. Mọi yêu cầu tiếp theo xác thực qua cookie này. Truy cập không có cookie hoặc sai token bị từ chối `401 Unauthorized`.
- **Đơn phiên (Single Instance)**: Chỉ cho phép một phiên bản `serve` chạy tại một thời điểm trên cùng thư mục dữ liệu (bảo vệ bằng file khóa `serve.lock` ghi PID và port). Khi khởi động phiên thứ hai, server từ chối ngay với thông báo nêu rõ PID và cổng đang chạy.
- **Dừng an toàn & Giám sát tiến trình**: Nhấn Ctrl+C hoặc đóng cửa sổ terminal (SIGHUP) sẽ gửi tín hiệu SIGINT tới serve và chuyển tiếp tín hiệu dừng tới toàn bộ các run do dashboard khởi chạy. Cơ chế an toàn chống mồ côi `PDEATHSIG` đảm bảo các tiến trình agent con tự động dừng nếu server đột ngột gặp sự cố.
- **Tương tác đa chiều với terminal**: Các run được khởi chạy từ terminal (ví dụ `agent-brain run lead`) sẽ tự động xuất hiện trực tiếp trên dashboard theo thời gian thực và người dùng có thể bấm nút **Kill** trên giao diện để dừng chúng an toàn.

### Năm màn hình chức năng
1. **Tổng quan (`#/`)**: Hiển thị bảng tóm tắt nhanh số lượng task theo từng nhóm trạng thái (đang chạy, chờ duyệt plan, cần chú ý, hoàn tất) và danh sách thẻ task gần đây. Hỗ trợ lọc nhanh theo profile (`Tất cả`, `work`, `personal`).
2. **Giao việc (`#/assign`)**: Ô nhập yêu cầu bằng tiếng Việt, chọn dự án thông qua ô tìm kiếm tự động gợi ý (tự động đề xuất slug chuẩn hóa). Nếu tên nhập vào bị mơ hồ hoặc trùng khớp nhiều dự án, giao diện hiển thị danh sách để người dùng chọn thay vì tự động chạy.
3. **Chi tiết task (`#/task/:id`)**:
   - Thanh tiến trình pipeline trực quan qua 3 vai trò Lead → Dev → QA (stepper).
   - Hiển thị nội dung kế hoạch `.plan.md` ở chế độ chỉ đọc (read-only). Người dùng chỉnh sửa plan trực tiếp bằng trình soạn thảo mã nguồn ngoài và commit với cú pháp `<slug>: plan`.
   - Các nút hành động theo ngữ cảnh quy trình: `Chạy Dev` (khi plan đã sẵn sàng và commit sạch), `Chạy QA` (sau khi Dev hoàn thành), `Chạy lại Lead` (khi gặp Vướng mắc hoặc QA Cần sửa).
   - Nút **Kill** dừng ngay tiến trình khi cần.
   - Hộp log sự kiện trực tiếp qua Server-Sent Events (SSE) kèm tự động cuộn (auto-scroll) và nút chuyển chế độ xem dữ liệu thô (raw JSON).
4. **Thống kê (`#/stats`)**: Biểu đồ thanh CSS trực quan thống kê lượng token và chi phí USD tương đương theo từng ngày (cột flex), theo từng vai trò (lead, dev, qa, inspect), theo từng dự án và theo hồ sơ profile.
5. **Dự án (`#/projects`)**: Danh sách toàn bộ dự án đã đăng ký trong `projects_map.json`. Cho phép chỉnh sửa danh sách `aliases` (biệt danh) trực tiếp với kiểm tra trùng lặp và nút **Quét lại** (`agent-brain projects scan`) để tự động cập nhật danh mục repo.

### Quy tắc điều phối & Hồ sơ (Pipeline & Profile Rules)
- **Quy tắc pipeline**: Sau khi Lead chạy xong, task chuyển sang trạng thái `Chờ duyệt plan`. Các bước Dev và QA chỉ chạy khi người dùng chủ động bấm nút trên giao diện. Nội dung plan không chỉnh sửa trên web; nếu gặp `Vướng mắc` (dev:blocked) hoặc `Cần sửa` (qa:fix), giao diện cung cấp nút bấm để chạy lại Lead hoặc tiếp tục Dev sau khi người dùng sửa file và commit.
- **Ràng buộc một run mỗi dự án**: Không cho phép khởi chạy song song hai tác vụ trên cùng một dự án. Nút chạy sẽ bị vô hiệu hóa kèm lý do thông báo nếu dự án đang bận.
- **Ràng buộc tài khoản cá nhân (Personal Profile)**: Tuân thủ quy tắc E-06, toàn hệ thống chỉ cho phép tối đa 1 tác vụ chạy với profile `personal` tại một thời điểm để tránh xung đột `~/.claude-orca`.
- **Bộ chuyển đổi Profile**: Bộ chọn profile trên giao diện (`Tất cả` / `work` / `personal`) chỉ có tác dụng lọc tầm nhìn (view filter) cho danh sách task, thống kê và bộ gợi ý dự án. Một task mới khi tạo sẽ luôn chạy dưới profile được cấu hình sẵn của dự án đó trong `projects_map.json`, giao diện không cho phép ghi đè profile khi giao việc.

### Danh mục HTTP API
Mọi route đều yêu cầu xác thực cookie phiên hợp lệ (trừ `GET /?token=TOKEN` cấp cookie):
- `GET /api/tasks?profile=work|personal`: Danh sách task và trạng thái bận (`busy`) của các dự án.
- `POST /api/tasks`: Tạo task mới và khởi chạy Lead (`{ "project": "...", "request": "...", "slug": "..." }`).
- `GET /api/tasks/:id`: Chi tiết task, trạng thái các vai trò, nội dung plan, commit sha và cờ `plan_dirty`.
- `POST /api/tasks/:id/lead`: Chạy lại bước Lead cho task.
- `POST /api/tasks/:id/dev`: Khởi chạy bước Dev.
- `POST /api/tasks/:id/qa`: Khởi chạy bước QA.
- `GET /api/runs/:id/events`: Lấy danh sách toàn bộ sự kiện lịch sử của một run từ SQLite.
- `GET /api/runs/:id/stream`: Luồng sự kiện thời gian thực Server-Sent Events (SSE).
- `POST /api/runs/:id/kill`: Dừng an toàn run đang chạy theo mã `run_id`.
- `GET /api/projects?profile=work|personal`: Danh mục dự án kèm mã hash `etag`.
- `PUT /api/projects/:id`: Cập nhật aliases dự án (`{ "aliases": [...], "etag": "..." }`).
- `POST /api/projects/scan`: Quét lại các thư mục dự án trên ổ đĩa.
- `GET /api/projects/suggest?q=...&profile=...`: Gợi ý dự án xếp hạng theo độ khớp tên và alias.
- `GET /api/projects/:id/status`: Đường dẫn nhanh kiểm tra trạng thái git của dự án.
- `GET /api/stats?profile=...&since=...`: Báo cáo thống kê token và chi phí tương đương.
- `GET /api/stats/days?profile=...&since=...`: Thống kê token nhóm theo từng ngày (Local timezone).
- `GET /api/stats/profiles?since=...`: Thống kê phân bổ theo từng profile.
- `GET /api/slug?text=...`: Chuẩn hóa văn bản thành slug hợp lệ (hỗ trợ tiếng Việt có dấu và chữ đ).

### Tài nguyên tĩnh & Tái dựng CSS ngoại tuyến
- Tất cả tài nguyên (HTML shell, các phân mảnh màn hình, thư viện Alpine CSP, mã JavaScript và stylesheet `app.css`) đều được nhúng trực tiếp vào binary qua `include_bytes!`. Khi binary xuất xưởng, máy đích không cần kết nối mạng hay cài đặt thêm file nào.
- File stylesheet `agent-brain/assets/dashboard/app.css` được biên dịch sẵn và commit vào kho mã nguồn.
- Lệnh phát triển tái dựng CSS (chỉ dùng khi phát triển giao diện):
  ```bash
  bash scripts/build-css.sh
  ```
  Sử dụng binary độc lập Tailwind v4 trong `agent-brain/tools/tailwindcss` (không cần Node hay npm). Cờ `--check` giúp kiểm tra tính đồng bộ của file CSS trong CI/CD.
- Kiểm tra khói toàn diện dashboard:
  ```bash
  bash scripts/dashboard-smoke.sh
  ```
- Tham khảo thêm file `agent-brain/assets/dashboard/VENDOR.md` về nguồn gốc và cách thức đóng gói thư viện Alpine.js bản CSP.

### Cảnh báo an toàn (Security Notes)
- **Quyền hạn mức shell**: Token dashboard cấp toàn quyền thực thi mã nguồn và lệnh trên các thư mục dự án đã đăng ký. **Tuyệt đối không chia sẻ URL chứa token** cho người khác hoặc đưa vào nhật ký công khai.
- **Không cách ly cổng cho cookie (Port Isolation)**: Theo tiêu chuẩn RFC 6265 §8.5, cookie không được cách ly theo số cổng trên cùng một tên miền. Do đó, nếu máy tính đang chạy một dịch vụ web nội bộ khác trên `127.0.0.1`, dịch vụ đó về mặt lý thuyết có thể nhận được cookie phiên của dashboard.
- **Chỉ lắng nghe Loopback**: Dashboard chỉ mở trên giao diện cục bộ `127.0.0.1`. Các thiết bị khác trong mạng LAN hoàn toàn không thể kết nối tới server (được bảo vệ triệt để từ tầng socket bind).

### Xử lý sự cố (Troubleshooting)
- **Lỗi 401 Unauthorized**: Mở lại URL đầy đủ chứa `?token=TOKEN` được in ở terminal chạy `agent-brain serve` để làm mới cookie phiên trên trình duyệt.
- **Cổng bị chiếm dụng (Address in use)**: Mặc định server sử dụng cổng `7437`. Nếu cổng đã bị ứng dụng khác chiếm dụng, chỉ định cổng khác bằng cờ `--port <cổng>` (hoặc cấu hình khóa `dashboard_port` trong file `config.json`), hoặc dùng `--port 0` để hệ thống tự cấp cổng còn trống.
