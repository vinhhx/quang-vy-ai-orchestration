# Phase 2 — Lõi Rust (CLI, chưa có giao diện)

## 1. Mục tiêu

Chuyển script Phase 1 thành một binary Rust `agent-brain` có: quản lý tiến trình an toàn, bắt log stream-json, lưu token vào SQLite và fast-path không tốn token.

Giải quyết: #1 (tiến trình treo), #3 (phần bắt log), #6 (profile + thống kê token riêng), #9 (fast-path).

## 2. Phạm vi

**Trong phạm vi:** CLI, process runner, parser stream-json, SQLite, fast-path cấp 1 và 2, profile.

**Ngoài phạm vi:** web server, giao diện, SSE (Phase 4), tối ưu session (Phase 3).

## 3. Phân tích

### 3.1 Vì sao làm CLI trước UI
Phần khó nhất của Rust ở đây là quản lý tiến trình và đọc stream bất đồng bộ. Làm nó dưới dạng CLI giúp dễ test bằng terminal, không bị lẫn lỗi giao diện. Phase 4 chỉ cần gắn web server lên lõi đã chạy ổn.

### 3.2 Kill sạch cây tiến trình
`kill_on_drop` của Tokio chỉ kill tiến trình con trực tiếp. Claude CLI sinh thêm MCP server và shell con, nên cần:
- Cho tiến trình con chạy trong **process group riêng** khi spawn.
- Khi kill: gửi `SIGTERM` tới cả group, chờ vài giây, rồi `SIGKILL` cả group.
- Giữ `kill_on_drop` như lớp bảo vệ thứ hai.

### 3.3 Bắt log
Đọc stdout theo từng dòng, parse JSON, phân loại: khởi tạo (có `session_id`), tin nhắn assistant, tool call, kết quả tool, `result` cuối (có usage). Dòng không parse được thì vẫn lưu dạng thô, không làm sập chương trình.

### 3.4 Fast-path 3 cấp
| Cấp | Cách làm | Token |
|---|---|---|
| 1. Zero-token | Rust tự chạy `git status`, `git log`, `git diff --stat` | 0 |
| 2. 1-turn inspector | Rust gom `git diff` vào prompt, chạy `--max-turns 1`, không lưu session | Thấp |
| 3. Warm reuse | Hỏi tiếp vào session vừa chạy (`--resume`) | Để Phase 3 |

## 4. Thiết kế

### 4.1 Lệnh CLI
```
agent-brain projects scan
agent-brain run <role> "<dự án>" "<yêu cầu>"   # role: lead | dev | qa
agent-brain status "<dự án>"                   # fast-path cấp 1
agent-brain inspect "<dự án>"                  # fast-path cấp 2
agent-brain kill <task_id>
agent-brain stats [--profile work|personal]
```

### 4.2 Bảng SQLite (bản đầu)
```
projects(id, path, name_vi, aliases_json, type, profile)
tasks(id, project_id, title, branch, status, created_at)
runs(id, task_id, role, engine, profile, session_id, pid, status,
     started_at, ended_at, input_tokens, output_tokens,
     cache_read_tokens, cache_write_tokens, cost_usd)
events(id, run_id, ts, kind, raw_json)
```

### 4.3 Cấu trúc module
```
src/
  main.rs        # parse lệnh
  projects.rs    # quét + so khớp tên (port từ Phase 1)
  runner.rs      # spawn, process group, kill
  stream.rs      # đọc + parse stream-json
  db.rs          # SQLite
  fastpath.rs    # git status/diff, inspector
  profile.rs     # chọn CLAUDE_CONFIG_DIR / biến môi trường engine
```

## 5. Công việc

- [ ] Khởi tạo dự án Rust, thêm Tokio, rusqlite (bundled), serde_json, clap
- [ ] `projects.rs`: đọc `projects_map.json`, so khớp không dấu, Fallback Ask
- [ ] `runner.rs`: spawn trong process group riêng, kill cả group
- [ ] `stream.rs`: đọc từng dòng, phân loại sự kiện, in ra terminal
- [ ] `db.rs`: tạo bảng, ghi runs và events
- [ ] `fastpath.rs`: status và inspect
- [ ] `profile.rs`: chọn profile theo dự án
- [ ] Chuyển 3 prompt từ Phase 1 sang dùng qua binary

## 6. Tiêu chí nghiệm thu (test case)

| ID | Kịch bản | Kết quả mong đợi |
|---|---|---|
| P2-01 | `run lead` trên dự án thật | Log hiện từng bước trên terminal |
| P2-02 | Kill khi agent đang chạy `sleep 300` | Sau 5 giây, `ps` không còn `claude` hay `sleep` liên quan |
| P2-03 | Đóng terminal (Ctrl+C) giữa chừng | Không còn tiến trình sót |
| P2-04 | Run kết thúc | Bảng `runs` có đủ token, session_id, thời gian |
| P2-05 | Dòng JSON lỗi trong stream | Lưu thô, chương trình không dừng |
| P2-06 | `status` | Trả kết quả git, không gọi AI, 0 token |
| P2-07 | `inspect` | Đúng 1 turn, không tạo session lưu lại |
| P2-08 | `stats --profile work` | Chỉ tính các run của profile work |
| P2-09 | Copy binary sang máy/thư mục khác | Chạy được, không cần cài thêm gì (trừ CLI agent) |

## 7. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Bạn chưa đọc được Rust nên khó debug | Yêu cầu agent viết test cho từng module; mỗi module nhỏ, review theo hành vi qua test case |
| Định dạng stream-json thay đổi theo phiên bản CLI | Lưu `raw_json`, parse mềm (bỏ qua trường lạ) |
| Lỗi async khó hiểu | Giao cho agent từng module một, chạy test xong mới sang module tiếp |

## 8. Cách giao cho agent

Dùng chính giao thức Phase 1: mỗi module là một task, Lead viết plan, Dev code, QA đọc diff. Bạn nghiệm thu bằng bảng test case ở mục 6.
