# Phase 5 — Đa đội hình & nhánh Media

## 1. Mục tiêu

- Cho phép mỗi vai trò (Lead / Dev / QA) chạy bằng engine khác nhau, đổi được giữa task khi hết quota.
- Tách nhánh sản xuất video khỏi nhánh code, nhưng vẫn điều phối chung một nơi.

Giải quyết: #4 (media làm nặng Orca/Git worktree), #10 (phụ thuộc một hãng AI).

## 2. Phân tích

### 2.1 Vì sao routing hoạt động được
Nhờ giao thức Phase 1, các vai trò chỉ trao đổi qua file (`.plan.md`, `git diff`, `.qa-report.md`), không cần chung session. Vì vậy đổi engine cho một vai trò không ảnh hưởng vai trò khác.

### 2.2 Các engine
| Engine | Cách gọi | Phụ thuộc Phase 0 |
|---|---|---|
| Claude (work / personal) | `claude` + `CLAUDE_CONFIG_DIR` | S1, S5 |
| DeepSeek | `claude` + `ANTHROPIC_BASE_URL` + key DeepSeek | S6 |
| Antigravity | Tùy kết quả S7 | S7 |

Nếu S7 cho thấy Antigravity không chạy headless: chỉ dùng nó thủ công (bạn mở và giao `.plan.md`), dashboard theo dõi qua `git diff` thay vì log.

### 2.3 Chuyển engine khi hết quota
- Nhận diện lỗi quota/rate limit từ stream hoặc mã thoát.
- Không tự động chuyển ngầm: hỏi xác nhận (hoặc theo luật bạn cấu hình sẵn), vì chất lượng các engine khác nhau.
- Vì vai trò giao tiếp qua file, engine mới chỉ cần đọc `.plan.md` / `.handover.md` để tiếp tục.

### 2.4 Nhánh Media
- File video nặng hàng GB không đưa vào Git worktree của Orca.
- Dự án `type: media` chạy trong thư mục riêng (`/video-studio`), công cụ là FFmpeg / Remotion / Python.
- Agent chỉ viết script và cấu hình render; phần render là tiến trình riêng do binary quản lý (dùng lại runner Phase 2, có kill).
- Nối với pipeline kịch bản whiteboard hiện có: kịch bản đã duyệt → đầu vào cho bước dựng.

## 3. Thiết kế

### 3.1 Cấu hình đội hình (`teams.json`)
```json
{
  "teams": {
    "mac-dinh":    { "lead": "claude-work", "dev": "claude-work", "qa": "claude-work" },
    "tiet-kiem":   { "lead": "claude-work", "dev": "deepseek",    "qa": "claude-work" },
    "ca-nhan":     { "lead": "claude-personal", "dev": "deepseek", "qa": "claude-personal" }
  },
  "engines": {
    "claude-work":     { "cmd": "claude", "env": { "CLAUDE_CONFIG_DIR": "~/.claude-work" } },
    "claude-personal": { "cmd": "claude", "env": { "CLAUDE_CONFIG_DIR": "~/.claude-max" } },
    "deepseek":        { "cmd": "claude", "env": { "ANTHROPIC_BASE_URL": "<endpoint>", "ANTHROPIC_AUTH_TOKEN": "<key>" } }
  }
}
```
Không lưu key trực tiếp trong file nếu file có thể bị commit; đọc từ biến môi trường.

### 3.2 Luồng Media
```
Kịch bản đã duyệt ─► agent viết cấu hình render ─► render (FFmpeg/Remotion) ─► file output
                                                         ▲
                                         runner Phase 2 (theo dõi, kill)
```

### 3.3 Bổ sung SQLite
```
runs.engine        -- đã có, giờ dùng thật
media_jobs(id, project_id, script_ref, status, output_path, started_at, ended_at)
```

## 4. Công việc

- [ ] Đọc `teams.json`, chọn engine theo vai trò
- [ ] Nhận diện lỗi quota, đề xuất đổi đội hình
- [ ] Thống kê token theo engine
- [ ] Phương án Antigravity theo kết quả S7
- [ ] Thư mục và runner cho dự án media
- [ ] Nối đầu ra pipeline kịch bản sang bước dựng

## 5. Tiêu chí nghiệm thu (test case)

| ID | Kịch bản | Kết quả mong đợi |
|---|---|---|
| P5-01 | Chạy task với đội hình `tiet-kiem` | Lead/QA chạy Claude, Dev chạy DeepSeek; log ghi đúng engine |
| P5-02 | Giả lập hết quota giữa task | Hệ thống báo lỗi quota, đề xuất đổi, không tự đổi ngầm |
| P5-03 | Đổi engine giữa task | Engine mới tiếp tục từ `.plan.md`, không làm lại từ đầu |
| P5-04 | Thống kê theo engine | Tách đúng số token từng engine |
| P5-05 | Dự án media | Không tạo Git worktree, file render nằm ngoài repo |
| P5-06 | Kill job render | FFmpeg dừng, không còn tiến trình sót |
| P5-07 | Key DeepSeek | Không xuất hiện trong file đã commit hay trong log |

## 6. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Engine rẻ hơn viết code kém hơn | QA luôn chạy bằng engine mạnh; ghi tỉ lệ "Cần sửa" theo engine để so sánh |
| Định dạng log khác nhau giữa engine | Parser mềm, lưu `raw_json` |
| Lộ key trong log | Lọc biến môi trường nhạy cảm trước khi ghi log |
