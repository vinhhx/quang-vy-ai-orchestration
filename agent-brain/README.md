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
  "inspect_max_diff_bytes": 100000
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

---

## 13. Giả định CLI Claude Code mở

- **Giả định A1 (`--max-turns`)**: Cờ `--max-turns` không xuất hiện trong `claude --help` phiên bản 2.1.301 nhưng vẫn được CLI chấp nhận và xử lý giới hạn lượt ở chế độ non-interactive. Giả định này được đưa vào kiểm chứng thực tế trong tài liệu nghiệm thu `LIVE-CHECKS.md`.
