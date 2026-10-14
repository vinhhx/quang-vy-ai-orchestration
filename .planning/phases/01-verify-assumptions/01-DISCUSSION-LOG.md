# Phase 1: Verify Assumptions - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-14
**Phase:** 1-Verify Assumptions
**Areas discussed:** Cách chạy & lưu bằng chứng, Tài khoản & môi trường thử, Ngưỡng Đạt cho spike mơ hồ

---

## Cách chạy & lưu bằng chứng

| Option | Description | Selected |
|--------|-------------|----------|
| Tay + script ghi lại | Tự gõ, mỗi spike có file .sh ghi lệnh | |
| Hoàn toàn tay | Đúng tài liệu gốc, không script | |
| Agent chạy, bạn kết luận | Agent thực thi lệnh, người dùng kết luận | ✓ |

| Option | Description | Selected |
|--------|-------------|----------|
| Commit vào repo | Output thô làm fixture cho Phase 3, lọc dữ liệu nhạy cảm | ✓ |
| Chỉ trích đoạn | Chỉ dán vài dòng tiêu biểu | |
| Không lưu output | Chỉ ghi quan sát | |

| Option | Description | Selected |
|--------|-------------|----------|
| spikes/ ở gốc repo | spikes/spike-results.md + spikes/s1..s8 | ✓ |
| Trong .planning/phases/01-… | Gắn với phase GSD, bị archive khi dọn | |
| docs/spikes/ | Tài liệu dự án lâu dài | |

| Option | Description | Selected |
|--------|-------------|----------|
| Điền sẵn, để trống kết luận | Agent ghi số liệu, kết luận để TODO | |
| Agent đề xuất, bạn duyệt | Agent ghi cả kết quả đề xuất, đánh dấu chờ duyệt | ✓ |
| Agent không ghi gì | Chỉ lưu output thô | |

**Notes:** Lệch so với §6 tài liệu gốc ("tự chạy tay") — agent được chạy lệnh, người dùng vẫn là người chốt.

---

## Tài khoản & môi trường thử

| Option | Description | Selected |
|--------|-------------|----------|
| Tài khoản đang dùng hiện tại | ~/.claude mặc định | ✓ |
| Chạy trên cả 2 gói | So sánh Work vs Max | |
| API key trả theo token | Số token rõ, không phản ánh subscription | |

| Option | Description | Selected |
|--------|-------------|----------|
| Có đủ 2 tài khoản | Tạo ~/.claude-work, ~/.claude-max | |
| Mới có 1 tài khoản | Tách thư mục config cùng 1 tài khoản | |
| Hoãn S5 | Chờ chính sách công ty | |

**User's choice (free text):** "có sẵn claude và claude-orca có thể clone claude-orca sang claude-personal còn claude coi là claude-work"

| Option | Description | Selected |
|--------|-------------|----------|
| Có key, đặt qua biến môi trường | export DEEPSEEK_API_KEY | |
| Chưa có key | S6 ghi chưa thử | |
| Key trong file .env | spikes/.env gitignore | |

**User's choice (free text):** "deepseek đang dùng deepseek harness trong docker" — tìm thấy container `dsh-deepseek-harness`.

| Option | Description | Selected |
|--------|-------------|----------|
| Thử harness Docker hiện có | docker exec, stream-json, sửa file, kill | ✓ |
| Cả hai cách | Harness + host ANTHROPIC_BASE_URL | |
| Chỉ cách gốc | host + BASE_URL | |

| Option | Description | Selected |
|--------|-------------|----------|
| Máy Linux này | Linux trước | ✓ |
| Linux + macOS | | |
| Linux + Windows | | |

---

## Ngưỡng Đạt cho spike mơ hồ

| Option | Description | Selected |
|--------|-------------|----------|
| Chỉ cache_read token | Đạt nếu 1 phút > 0, 10 phút ≈ 0 | |
| Token + quan sát quota | So % usage | |
| Lặp nhiều lần lấy trung bình | 3 lần mỗi mốc | |

**User's choice (free text):** "Cách 10 phút ≈ 0" có thể không bao giờ xảy ra — system prompt và tool definitions của Claude Code giống nhau ở mọi session, có session khác cùng tài khoản thì vẫn cache. Đo phần riêng dự án (.gsd): cache ấm → cache_read, hết hạn → cache_creation_input_tokens. Đừng mặc định TTL 5 phút — có thể là 1 giờ trên subscription; thêm mốc ~65 phút.

| Option | Description | Selected |
|--------|-------------|----------|
| Đọc docs + thử nếu đã cài | Không cài mới | ✓ |
| Chỉ đọc docs | | |
| Cài và thử thật | | |

| Option | Description | Selected |
|--------|-------------|----------|
| Đóng nếu có phương án thay thế | S1, S4 bắt buộc rõ | ✓ |
| Chặt | Workaround phải được thử | |
| Lỏng | Chỉ cần có kết quả | |

| Option | Description | Selected |
|--------|-------------|----------|
| Ghi hiện trạng + thử kill process group | setsid + kill -- -PGID | ✓ |
| Chỉ ghi hiện trạng | | |
| Thêm cả MCP + Docker | | |

---

## Claude's Discretion

- Cấu trúc file trong `spikes/sN/` và cách lọc dữ liệu nhạy cảm.
- Task cụ thể cho S3.
- Cách gọi harness Docker cho S6.

## Deferred Ideas

- Chính sách công ty (SPK-10) — không thảo luận, vẫn là blocker.
- Kill có MCP server con / Docker đầy đủ.
- Hỗ trợ macOS/Windows.
