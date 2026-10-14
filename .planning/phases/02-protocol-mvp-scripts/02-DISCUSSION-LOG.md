# Phase 2: Protocol MVP (Scripts) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-14
**Phase:** 2-Protocol MVP (Scripts)
**Areas discussed:** Script & nơi đặt cấu hình, Chế độ chạy & giới hạn tool, Nhánh/file giao thức & nhật ký, Định tuyến & quét dự án

---

## Script & nơi đặt cấu hình

| Option | Description | Selected |
|--------|-------------|----------|
| Bash + jq, bỏ dấu bằng Python | Shell script, helper Python cho Unicode | |
| Toàn bộ Python | Gọn, dễ test, bỏ đi sau Phase 3 | |
| Toàn bộ Bash | Chỉ jq/iconv, bỏ dấu và merge JSON vụng | |

**User's choice (lần 1):** "Tech stack là rust thì nên dùng gì" → hỏi lại:

| Option | Description | Selected |
|--------|-------------|----------|
| Python, bỏ đi sau Phase 3 | Nhanh nhất; giá trị giữ lại là prompt, mẫu plan, schema, test case | ✓ |
| Bash + helper Python | Đúng chữ "shell script" của spec | |
| Viết Rust luôn từ Phase 2 | Không có code bỏ đi nhưng trái nguyên tắc roadmap, lấn Phase 3 | |

| Option | Description | Selected |
|--------|-------------|----------|
| Code trong repo, map ở ~/.config | Script + prompt trong repo, map ở ~/.config/agent-brain/ | ✓ |
| Tất cả trong repo | Map commit cùng code | |
| Tất cả ở ~/.config/agent-brain | Mất lịch sử git của script | |

| Option | Description | Selected |
|--------|-------------|----------|
| /mnt/data/IDE | Thư mục chứa repo này trên máy | |
| ~/projects | Như spec | |
| Nhiều thư mục gốc | root thành mảng | |

**User's choice:** "Mỗi dự án sẽ lưu path và cả repository trong database để quản lý" → hỏi lại:

| Option | Description | Selected |
|--------|-------------|----------|
| Quét danh sách roots + `brain add <path>` | scan_roots + add cho dự án ngoài roots | ✓ |
| Chỉ `brain add <path>` | Đăng ký thủ công | |
| Chỉ quét roots | Đúng spec, không có add | |

| Option | Description | Selected |
|--------|-------------|----------|
| URL git remote, tự đọc từ origin | null nếu không có remote | ✓ |
| Nhãn tự đặt | Như "p.zamba" trong spec | |
| Cả hai | repo_url + nhãn | |

| Option | Description | Selected |
|--------|-------------|----------|
| Một lệnh `brain` với subcommand | Giống CLI agent-brain Phase 3 | ✓ |
| Mỗi vai trò một script | Lặp code định tuyến/profile | |

---

## Chế độ chạy & giới hạn tool

| Option | Description | Selected |
|--------|-------------|----------|
| Cả 3 headless `claude -p`, lưu stream-json | Đo được, giống Phase 3 | ✓ |
| Lead tương tác, Dev/QA headless | Chỉnh plan bằng hội thoại | |
| Cả 3 tương tác | Không có log máy đọc được | |

| Option | Description | Selected |
|--------|-------------|----------|
| Allowlist sinh từ bảng Thay đổi | Read/Edit đúng file trong plan, cấm Grep/Glob, Bash chỉ git add/commit | ✓ |
| Như trên + cho Dev chạy test | Bash cho lệnh "Cách kiểm tra" | |
| Chỉ cấm Grep/Glob, Read tự do | Dựa vào prompt | |

| Option | Description | Selected |
|--------|-------------|----------|
| Read/Grep/Glob + chỉ ghi .plan.md | Bash chỉ git đọc | ✓ |
| Thêm Bash đọc tự do | Rủi ro Bash sửa file | |

| Option | Description | Selected |
|--------|-------------|----------|
| Lead Opus, Dev/QA Sonnet | Cấu hình được | ✓ |
| Cùng một model | Mặc định tài khoản | |
| Bạn quyết | Planner chọn | |

---

## Nhánh, file giao thức & nhật ký

| Option | Description | Selected |
|--------|-------------|----------|
| `task/<slug>` từ HEAD hiện tại, dừng nếu bẩn | Ghi base vào .plan.md | ✓ |
| Luôn từ nhánh mặc định | Không nối tiếp task được | |
| Không tạo nhánh | Lệch spec | |

| Option | Description | Selected |
|--------|-------------|----------|
| Gốc dự án, commit trên nhánh task | Commit plan sau Lead; QA diff từ commit plan | ✓ |
| Thư mục `.brain/` bị gitignore | Không bẩn lịch sử | |
| Gốc dự án, không commit | Dễ bị Dev commit kéo theo | |

| Option | Description | Selected |
|--------|-------------|----------|
| Bạn quyết tay | Script chỉ báo | ✓ |
| `brain dev --fix` đọc .qa-report.md | Lệch giao thức | |

| Option | Description | Selected |
|--------|-------------|----------|
| Script tự điền số liệu, bạn điền nhận xét | Từ log stream-json, lưu protocol-log/ | ✓ |
| Ghi tay hoàn toàn | Theo §7 spec | |

---

## Định tuyến & quét dự án

| Option | Description | Selected |
|--------|-------------|----------|
| Thư mục con trực tiếp có `.git` | Độ sâu 1, media dùng add, mất thì đánh dấu missing | ✓ |
| Đệ quy tới độ sâu N | Dễ quét nhầm submodule | |
| Mọi thư mục con trực tiếp | Dễ lẫn rác | |

| Option | Description | Selected |
|--------|-------------|----------|
| Khớp đúng sau chuẩn hóa | Bỏ dấu, chữ thường, khoảng trắng/-/_ như nhau | ✓ |
| Khớp đúng, nếu 0 thì thử chứa chuỗi | Tiện hơn, dễ mơ hồ | |

| Option | Description | Selected |
|--------|-------------|----------|
| In danh sách, thoát mã lỗi | Không hỏi tương tác | ✓ |
| Hỏi chọn số nếu đang ở terminal | Thêm nhánh code | |

| Option | Description | Selected |
|--------|-------------|----------|
| Trống, bắt buộc đặt tay | Từ chối chạy khi chưa có profile | ✓ |
| Mặc định `work` | Chạy được ngay | |

---

## Claude's Discretion

- Cấu trúc module Python, tên file prompt, cách cài symlink.
- Giới hạn tool chi tiết của QA; cách Lead xử lý khi thiếu `.gsd`; ai thực hiện commit của Dev; ví dụ plan tốt trong prompt Lead; cách test P1-01..P1-10.

## Deferred Ideas

- Hỏi chọn số tương tác khi Fallback Ask.
- `brain dev --fix` đọc `.qa-report.md`.
- Quét đệ quy nhiều cấp.
- SPK-10 chính sách công ty (blocker mở từ Phase 1).
