# brain — Bộ công cụ điều phối giao thức MVP (Protocol MVP)

`brain` là công cụ dòng lệnh (CLI) tạm thời thuộc Phase 2 của dự án QuangVy-Orchestration, phục vụ chứng minh giao thức 3 vai trò (Lead → Dev → QA) trên các dự án phần mềm thật trước khi chuyển giao sang binary Rust `agent-brain` ở Phase 3.

---

## 1. Cài đặt

- **Yêu cầu môi trường**: Python ≥ 3.9 (chỉ dùng thư viện chuẩn Python stdlib, không cài thêm package ngoại vi nào), bash trên Linux.
- **Cài đặt symlink**:
  ```bash
  bash brain/install.sh
  ```
  Lệnh sẽ tạo symlink từ `brain/brain` vào `${BRAIN_BIN_DIR:-$HOME/.local/bin}/brain`. Đảm bảo thư mục này có trong `$PATH`.
- **Gỡ cài đặt**:
  ```bash
  bash brain/install.sh --uninstall
  ```
  Script từ chối ghi đè hoặc gỡ file không phải là symlink trỏ tới repo này.

---

## 2. Quản lý danh mục dự án (`projects_map.json`)

- **Vị trí lưu trữ**: `~/.config/agent-brain/projects_map.json` (nằm ngoài repo, không commit bí mật hay đường dẫn máy cá nhân lên GitHub).
- **Cấu trúc dữ liệu (D-04)**:
  ```json
  {
    "scan_roots": ["/home/user/projects"],
    "projects": [
      {
        "id": "web-ban-hang",
        "path": "/home/user/projects/web-ban-hang",
        "name_vi": "Web bán hàng",
        "aliases": ["shop", "ban hang"],
        "type": "code",
        "repo": "git@github.com:example/web.git",
        "profile": "work",
        "missing": false
      }
    ]
  }
  ```
- **Các trường chỉnh sửa tay (D-05, D-07)**:
  - `name_vi`: Tên hiển thị tiếng Việt có dấu.
  - `aliases`: Danh sách tên viết tắt, biệt danh.
  - `profile`: Bắt buộc điền `"work"` hoặc `"personal"`. `brain` sẽ từ chối chạy agent nếu dự án chưa được cấu hình profile.
  Khi quét lại bằng `brain scan`, các trường sửa tay trên không bao giờ bị ghi đè.

---

## 3. Các lệnh chính

### `brain scan`
Quét thư mục gốc ở độ sâu 1 để tự động nhận diện các dự án có thư mục `.git`:
```bash
brain scan --root ~/projects --root ~/work
```

### `brain add`
Đăng ký thủ công một thư mục dự án (cho phép loại `code` hoặc `media` không dùng git):
```bash
brain add /path/to/media-project --type media
```

### `brain lead`
Khảo sát dự án và sinh kế hoạch sửa đổi `.plan.md` trên nhánh mới `task/<slug>`:
```bash
brain lead "Web bán hàng" "Thêm bộ lọc tìm kiếm theo giá" --name "filter-by-price"
brain lead "shop" "Khảo sát tính năng" --dry-run
```
> [!IMPORTANT]
> **Lưu ý về Re-plan**: Nếu chạy lại `brain lead` trên cùng một nhánh task, `brain` sẽ tái sinh `.plan.md` từ skeleton mới và ghi đè các sửa đổi tay chưa commit trên `.plan.md`. Để giữ lại các sửa đổi tay, hãy commit chúng bằng lệnh:
> ```bash
> git -C <root> commit -m "<task>: plan" -- .plan.md
> ```

### `brain dev`
Chạy Dev để thực thi các sửa đổi được liệt kê trong bảng `Thay đổi` của `.plan.md`:
```bash
brain dev "web-ban-hang"
brain dev "shop" --dry-run
```

### `brain qa`
Chạy QA kiểm tra chất lượng trên `git diff` giữa commit plan và HEAD:
```bash
brain qa "web-ban-hang"
brain qa "shop" --dry-run
```

---

## 4. Định tuyến & Fallback Ask

- **Quy tắc so khớp (D-08)**:
  - Chuẩn hóa chuỗi tìm kiếm và dữ liệu: chuyển chữ thường, bỏ dấu tiếng Việt (kể cả `đ/Đ` → `d`), chuẩn hóa Unicode NFC, gộp khoảng trắng, dấu gạch nối `-` và `_`.
  - So khớp chính xác sau chuẩn hóa với `id`, `name_vi` và từng alias. Không dùng substring hay fuzzy search.
  - Nếu query khớp chính xác với `id` của dự án, `id` luôn được ưu tiên giải quyết xung đột alias.
- **Fallback Ask (D-09)**:
  - Nếu khớp 0 dự án hoặc > 1 dự án: `brain` in danh sách gợi ý ra stdout và **thoát ngay với mã lỗi 2**, tuyệt đối không tự ý chạy agent.

---

## 5. Bảng mã thoát (Exit Codes)

| Mã | Tên | Ý nghĩa |
|---|---|---|
| `0` | EXIT_OK | Thành công hoàn toàn (hoặc QA kết luận Đạt/Cần sửa hợp lệ). |
| `1` | EXIT_ERROR | Lỗi thực thi, cấu hình không hợp lệ, thiếu file, vi phạm tiền điều kiện. |
| `2` | EXIT_ROUTING | Fallback Ask: Không khớp hoặc khớp nhiều hơn 1 dự án (in danh sách để người dùng chọn). |
| `3` | EXIT_PROTOCOL | Vi phạm giao thức: Sửa file ngoài plan (ROLE-01, ROLE-06). |
| `4` | EXIT_BLOCKED | Vướng mắc: Dev gặp trở ngại hoặc thiếu thông tin, ghi Vướng mắc vào `.plan.md` và dừng lại an toàn. |
| `130` | EXIT_INTERRUPTED | Bị ngắt bởi người dùng qua tín hiệu bàn phím (Ctrl+C). |

---

## 6. Cấu hình (`brain/config.json`)

Mặc định đọc tại `brain/config.json`. Có thể ghi đè đường dẫn bằng biến môi trường `BRAIN_CONFIG`.

Các khóa cấu hình chính:
- `claude_bin`: Đường dẫn thực thi Claude Code (mặc định `"claude"`).
- `models`: Model chỉ định theo vai trò: `{"lead": "opus", "dev": "sonnet", "qa": "sonnet"}` (D-13).
- `profiles`:
  - `work`: `claude_config_dir: null` (bỏ đặt biến `CLAUDE_CONFIG_DIR`, dùng mặc định `~/.claude`).
  - `personal`: `claude_config_dir: "~/.claude-orca"` (dùng tài khoản cá nhân).
  - `unset_env`: Danh sách các biến bị xóa khỏi môi trường con (`ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`).
- `common_args`: Danh sách đối số chung cho mọi lệnh gọi Claude (ví dụ `["--strict-mcp-config"]`).
- `max_turns`: Giới hạn lượt cho từng vai trò (`{"lead": null, "dev": null, "qa": null}`).
- `timeout_seconds`: Thời gian chờ tối đa cho mỗi vai trò (lead 1800s, dev 1800s, qa 1200s).
- `runs_dir`: Thư mục lưu log thô (mặc định `~/.local/share/agent-brain/runs/`).
- `protocol_log_dir`: Thư mục lưu nhật ký nghiệm thu (mặc định `protocol-log/` trong repo này).

---

## 7. Giới hạn vai trò & 3 lớp bảo vệ

| Vai trò | Công cụ cho phép | Công cụ cấm |
|---|---|---|
| **Lead** | Read, Grep, Glob, Write(`.plan.md`), Edit(`.plan.md`), Bash git đọc | Không sửa bất kỳ file code nào |
| **Dev** | Read/Edit/Write(`.plan.md` + các file trong bảng Thay đổi), Bash(`git add *`, `git commit *`) | Grep, Glob, WebFetch, WebSearch, chạy test |
| **QA** | Read, Write(`.qa-report.md`), Bash git đọc | Không sửa bất kỳ file code nào |

**3 lớp thực thi bảo vệ**:
1. **Lớp 1 (Cờ quyền)**: `--allowedTools` và `--disallowedTools` giới hạn công cụ ở cấp CLI.
2. **Lớp 2 (Dev Guard Hook)**: PreToolUse hook cấu hình qua `--settings` chặn đọc/sửa bất kỳ file nào ngoài allowlist của plan.
3. **Lớp 3 (Hậu kiểm git)**: Kiểm tra `git diff` và `git status` sau khi agent kết thúc; nếu có file trái phép bị sửa, trả về mã thoát `3` (EXIT_PROTOCOL).

---

## 8. Log và nhật ký task

- **Log thô (`runs/`)**: Lưu tại `~/.local/share/agent-brain/runs/<project>/<task>/<timestamp>-<role>/` (quyền `0700`, không bao giờ commit vào repo để tránh lộ mã nguồn dự án).
- **Nhật ký task (`protocol-log/`)**: Lưu tại `protocol-log/<project>--{task}.md` (được commit vào repo). Chỉ chứa các con số dẫn xuất, đường dẫn tương đối, số token, kết luận và ghi chú của người dùng. Tuyệt đối không lưu nội dung file hay prompt vào đây.
- **Định nghĩa turn**: Một assistant turn được tính là một `message.id` riêng biệt trên dòng có `type == "assistant"`. "Số turn của Dev trước khi bắt đầu sửa" là số turn trợ lý diễn ra trước công cụ Edit/Write đầu tiên sửa một file không phải là file giao thức (`.plan.md`, `.qa-report.md`).
- **Lệnh kiểm tra cổng**:
  ```bash
  cd <repo root> && python3 -m brain.tasklog check protocol-log --min 5
  ```
  Lưu ý: Lệnh `python3 -m brain...` chỉ hoạt động khi chạy từ thư mục gốc của repo (`<repo root>`).
> [!CAUTION]
> **Không dogfood trên chính repo này**: Tuyệt đối không chạy `brain lead/dev/qa` trên repo QuangVy-Orchestration. Thư mục `protocol-log/` của repo sẽ bị thay đổi trong quá trình chạy và làm kích hoạt vi phạm hậu kiểm git (thoát mã 3); Phase 3 D-01 cũng nghiêm cấm dogfooding trên chính repo công cụ.

---

## 9. Hợp đồng cho Phase 3

Toàn bộ các prompt (`brain/prompts/*.md`), mẫu tài liệu (`brain/templates/*.md`), bộ dữ liệu kiểm thử hợp đồng (`tests/contract/*.json`) và các test case `test_p1_xx` là hợp đồng kỹ thuật bất biến để Phase 3 port nguyên vẹn sang Rust (`agent-brain`).

---

## 10. Giả định CLI & Đối chiếu với kết quả Phase 1 (Spikes)

| Mã | Giả định CLI | Spike | Trạng thái Phase 1 | Cấu hình thay đổi nếu sai |
|---|---|---|---|---|
| **A-01** | `claude -p --output-format stream-json` bắt buộc kèm `--verbose`. | S1 | **Đã xác nhận** (S1 bỏ `--verbose` lập tức lỗi mã 1). | Cố định trong `agent.py:build_command`. |
| **A-02** | Định dạng stream-json có các dòng `system/init`, `assistant`, `user` và dòng `result` mang `usage`, `permission_denials`. | S1 | **Đã xác nhận** (`spikes/s1/raw/s1.jsonl`). | Parser trong `brain/streamlog.py`. |
| **A-03** | `--allowedTools` và `--disallowedTools` nhận các pattern cách nhau bằng khoảng trắng. | S1/S4/S8 | **Một phần**: S1/S4 chỉ dùng một giá trị (`Bash(ls *)`, `Bash(sleep *)`); nhiều giá trị cách nhau bằng khoảng trắng và việc tool bị từ chối được ghi vào `permission_denials` ở chế độ `-p` chưa kiểm chứng (xác minh tại P1-06..P1-09). | `common_args` hoặc role spec. |
| **A-04** | Read, Grep, Glob không hỏi quyền mặc định, cần PreToolUse hook để chặn đọc tự do cho Dev. | — | **Chưa kiểm chứng** (Phase 1 không thử quyền của Read/Grep/Glob); theo tài liệu Claude Code. Guard hook là lớp chặn đọc (xác minh tại P1-07 qua `guard.jsonl`). | `brain/guard.py`. |
| **A-05** | `--settings <file>` kích hoạt PreToolUse hook chặn với mã 2 và trả stderr. | S1/P1-07 | **Chưa kiểm chứng trên CLI thật** (xác minh tại P1-07). | `extra_args` trong `role_dev.py`. |
| **A-06** | Profile work bỏ đặt `CLAUDE_CONFIG_DIR`, profile personal dùng `~/.claude-orca`. | S5/SPK-10 | **Một phần**: ánh xạ thư mục đã chốt (SPK-10); profile work bỏ đặt `CLAUDE_CONFIG_DIR` đã chạy thật (S5). Chưa kiểm chứng: chạy thật với profile personal, tách transcript, và `CLAUDE_CONFIG_DIR=~/.claude` đặt tường minh (E-07). | `profiles.work`, `profiles.personal`. |
| **A-07** | `--model` nhận alias `opus`, `sonnet`; `--strict-mcp-config` bỏ qua MCP của user. | — | **Chưa kiểm chứng**: Phase 1 không truyền `--model` hay `--strict-mcp-config` (S1 chạy model mặc định của tài khoản). Xác minh ở lần chạy thật đầu tiên. | `models.*`, `common_args`. |
| **A-08** | Cờ `--max-turns` tồn tại và giới hạn số lượt. | S8 | **Đã xác nhận** (`claude -p ... --max-turns 1`). | `max_turns.*` trong `config.json`. |
| **A-09** | SIGTERM tới PID claude dọn sạch các tiến trình con Bash. | S4 | **Đã xác nhận** (S4 SIGTERM tree-kill thành công, SIGKILL để sót con). | `timeout_seconds`. |
| **A-10** | Công cụ subagent tên là `Task`, có `MultiEdit`/`NotebookEdit`; đưa vào `--disallowedTools` một tên CLI không biết thì vô hại. | S1 | **Chưa kiểm chứng**: dòng `init` của S1 có liệt kê tên công cụ, nhưng việc cấm một tên không tồn tại chưa được thử. | `DEV_DISALLOWED_TOOLS`, `QA_DISALLOWED_TOOLS`, `LEAD_DISALLOWED_TOOLS`. |
| **A-11** | Biến môi trường `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1` giữ cwd của agent tại gốc repo sau lệnh Bash; cú pháp quyền đường dẫn tuyệt đối `//path` cho phép ghi file khi agent đổi thư mục. | — | **Chưa kiểm chứng tới lần chạy thật tiếp theo** (unverified until the next live run). | `brain/config.py:build_env`, `brain/agent.py:abs_rules`. |
| **E-06** | Dùng chung `~/.claude-orca` khi phiên agent khác đang chạy có nguy cơ xoay token (E-06). | S5 | **Chưa kiểm chứng** (cần chạy ngoài phiên). | Hướng dẫn vận hành người dùng. |
