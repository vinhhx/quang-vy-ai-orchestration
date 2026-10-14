# Phase 0 — Kiểm chứng giả định

## 1. Mục tiêu

Kiểm tra bằng thực tế mọi giả định kỹ thuật mà bảng kiến trúc dựa vào, trước khi viết dòng code Rust nào. Một giả định sai ở đây có thể làm hỏng cả một phase phía sau.

## 2. Phân tích

Bảng kiến trúc gốc dựa trên nhiều cờ CLI và hành vi chưa được thử trực tiếp. Một số điểm đã biết là cần chỉnh:

| Giả định trong bảng gốc | Thực tế cần kiểm tra |
|---|---|
| `--output-format stream-json` đủ để bắt log | Ở chế độ `-p` cần thêm `--verbose` |
| `kill_on_drop` hủy cả cây tiến trình | Chỉ kill tiến trình con trực tiếp; MCP server, bash con có thể còn sót |
| Fork session luôn ăn prompt cache | Cache sống khoảng 5 phút; session gốc nguội thì không giảm giá |
| Giảm 90% phí nhờ cache | Đúng với API trả theo token; với gói subscription, mức ảnh hưởng tới quota chưa rõ |
| Antigravity chạy được như CLI | Chưa xác nhận có chế độ headless + log JSON |
| DeepSeek dùng được qua endpoint tương thích Anthropic | Cần thử với chính Claude Code qua `ANTHROPIC_BASE_URL` |

## 3. Danh sách spike (thí nghiệm)

Mỗi spike ghi kết quả vào `spike-results.md` theo mẫu ở mục 5.

### S1 — Stream JSON
```bash
claude -p "liệt kê file trong thư mục này" --output-format stream-json --verbose > s1.jsonl
```
Kiểm tra:
- Có dòng chứa `session_id` không?
- Dòng cuối (`type: result`) có thông tin usage / token không?
- Mỗi tool call (đọc file, chạy bash) có xuất hiện thành dòng riêng không?

### S2 — Resume và fork
```bash
claude -p "nhớ số 42" --output-format json        # lấy session_id
claude -p "số tôi bảo nhớ là gì?" --resume <id>
claude -p "số đó cộng 1?" --resume <id> --fork-session
```
Kiểm tra: fork có tạo session_id mới không, session gốc có bị thay đổi không.

### S3 — Prompt cache
Chạy cùng một task 2 lần cách nhau 1 phút, rồi 2 lần cách nhau 10 phút. So sánh số token cache read trong dòng `result`.

### S4 — Kill cây tiến trình
Cho agent chạy một lệnh dài (ví dụ `sleep 300` qua bash). Kill tiến trình `claude`, sau đó chạy `ps -ef | grep sleep`. Ghi lại có tiến trình nào còn sót không.

### S5 — Tách profile
```bash
CLAUDE_CONFIG_DIR=~/.claude-work claude -p "hi"
CLAUDE_CONFIG_DIR=~/.claude-max  claude -p "hi"
```
Kiểm tra: hai profile có dùng đúng tài khoản riêng và lịch sử session riêng không.

### S6 — DeepSeek qua Claude Code
Đặt `ANTHROPIC_BASE_URL` và key của DeepSeek, chạy một task nhỏ có sửa file. Kiểm tra: tool call có hoạt động không, stream-json có cùng định dạng không.

### S7 — Antigravity headless
Tìm trong tài liệu chính thức xem có lệnh chạy không tương tác, có xuất JSON không, có cờ giới hạn turn không. Nếu không có, ghi rõ để Phase 5 thiết kế phương án khác.

### S8 — Max turns
```bash
claude -p "review git diff" --max-turns 1 --output-format json
```
Kiểm tra: dừng đúng 1 turn, kết quả có dùng được không.

## 4. Tiêu chí nghiệm thu

- [ ] Cả 8 spike có kết quả Đạt / Không đạt / Một phần.
- [ ] Mỗi kết quả "Không đạt" có ghi phương án thay thế.
- [ ] Đã quyết định profile Work/Personal dùng tài khoản nào (sau khi xác nhận chính sách công ty).

## 5. Mẫu ghi kết quả (`spike-results.md`)

```markdown
## S1 — Stream JSON
- Ngày thử:
- Lệnh:
- Kết quả: Đạt / Không đạt / Một phần
- Quan sát:
- Ảnh hưởng tới thiết kế:
```

## 6. Cách giao cho agent

Phase này nên tự chạy tay để hiểu hành vi của CLI. Có thể nhờ agent giải thích output khi đọc không hiểu, nhưng đừng để agent tự kết luận thay bạn.
