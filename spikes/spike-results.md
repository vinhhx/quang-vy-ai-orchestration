# Kết quả kiểm chứng giả định (Phase 1)

> Quy ước (D-01, D-02): agent chạy lệnh và ghi số liệu khách quan, rồi đề xuất kết quả kèm nhãn chờ duyệt đặt trong ngoặc đơn ngay sau kết quả. Chỉ người dùng được kết luận: sửa kết quả nếu cần rồi xóa nhãn. Phase chỉ đóng khi không còn nhãn nào (D-13).
> Máy chuẩn: Linux (D-08). Mọi output thô đã qua `spikes/lib/scrub.py` và `spikes/lib/scan-secrets.sh` trước khi commit (D-03).
> Phiên bản công cụ: claude (điền ở Task 8) | agy (điền ở Task 8) | docker (điền ở Task 8)
> Script chạy lại được: `spikes/sN/run.sh`; kiểm tra bằng chứng: `spikes/sN/verify.sh`; kiểm tra file này: `spikes/lib/check-results.sh`.

## S1 — Stream JSON

- Ngày thử: 2026-10-15
- Phiên bản: claude 2.1.300 (Claude Code), model claude-opus-5-5 (chạy lồng trong phiên agent, đã `env -u` các biến CLAUDE_CODE_*, tài khoản work `~/.claude`)
- Lệnh:
  - Dương tính: `cd <thư mục tạm có 3 file> && claude -p "liệt kê file trong thư mục này" --output-format stream-json --verbose --allowedTools "Bash(ls *)" </dev/null > spikes/s1/raw/s1.jsonl`
  - Âm tính (E-01): cùng lệnh nhưng bỏ `--verbose`
  - Chạy lại bằng `bash spikes/s1/run.sh`
- Kết quả: Đạt (chờ duyệt)
- Quan sát:
  - Dòng `system/init` có `session_id`, `model`, `claude_code_version`, `cwd`, danh sách tool/skill. Dòng cuối `type: result` (`subtype: success`, `num_turns: 2`, `terminal_reason: completed`) có `usage`, `session_id`, `total_cost_usd`, `modelUsage`.
  - Có đủ 9 dòng. Các cặp type/subtype: `system/hook_started`, `system/hook_response`, `system/init`, `rate_limit_event`, `assistant` x3, `user` x1, `result/success`. Dòng hook (SessionStart) đến TRƯỚC `init`; `rate_limit_event` là loại dòng không có trong tài liệu gốc (E-02, A13).
  - Mỗi tool call KHÔNG phải một dòng riêng theo nghĩa "một dòng = một lệnh": `tool_use` nằm trong khối `content` của một dòng `assistant`; kết quả là dòng `user` riêng chứa khối `tool_result` (1 `tool_use`, 1 `tool_result`).
  - Mỗi khối nội dung của một tin nhắn là một dòng `assistant` riêng nhưng dùng chung `message.id`: 3 dòng `assistant`, chỉ 2 `message.id` khác nhau (khối `thinking` và `text` cùng một id, cùng `usage`). `output_tokens` theo từng dòng chỉ là giá trị tạm (16, 6, 6) so với 293 ở dòng `result`.
  - `usage` ở dòng `result` là tổng cộng cả phiên: input 4, output 293, cache_read 30387, cache_creation 10482, trong đó `ephemeral_5m` 0 và `ephemeral_1h` 10482 (gói subscription xin TTL 1 giờ).
  - Bỏ `--verbose`: lỗi tức thì, không gọi API, stdout rỗng, stderr `Error: When using --print, --output-format=stream-json requires --verbose`, `exit_code=1` (A1 được xác nhận bằng thực nghiệm).
  - Dòng `rate_limit_event` báo mức dùng 7 ngày của tài khoản work là 0.94 (`allowed_warning`): các spike sau phải giữ prompt thật nhỏ.
- Ảnh hưởng tới thiết kế: Parser Phase 3 (LOG-01/02) phải (1) bắt buộc truyền `--verbose` khi dùng `-p --output-format stream-json`; (2) chịu được loại dòng lạ (`system/hook_*`, `rate_limit_event`) và dòng đến trước `init`, bỏ qua thay vì lỗi; (3) tìm `tool_use` trong `assistant.message.content[]` và `tool_result` trong dòng `user`; (4) khử trùng `usage` theo `message.id` nếu đọc theo từng dòng, và lấy token cuối cùng từ dòng `result` (`output_tokens` từng dòng chỉ là giá trị tạm).
- Phương án thay thế: Không cần (kết quả Đạt); nếu sau này đổi sang `--output-format json` thì mất log theo thời gian thực nên không dùng cho LOG-01.
- Phase bị ảnh hưởng: Phase 3
- Cách làm đã kiểm chứng (D-13): Lệnh spawn tạo ra dòng `init` và dòng `result` là `claude -p "<prompt>" --output-format stream-json --verbose --allowedTools "Bash(ls *)"` chạy từ thư mục làm việc của task với `</dev/null`, đã bỏ các biến `CLAUDECODE` và `CLAUDE_CODE_*` và bỏ `CLAUDE_CONFIG_DIR` (profile work mặc định). Bắt buộc có `--verbose`; không dùng `--bare` (mất đăng nhập subscription). Đọc stdout theo từng dòng JSON, lấy `session_id` từ `system/init`, lấy `usage` và `num_turns` từ dòng `type: result` cuối cùng.
- Bằng chứng:
  - `spikes/s1/raw/s1.jsonl`
  - `spikes/s1/raw/s1-no-verbose.txt`
  - `spikes/s1/raw/s1-version.txt`
  - `spikes/s1/raw/s1-observations.txt`
  - `spikes/s1/run.sh`

## S2 — Resume và fork

- Ngày thử: 2026-10-15
- Phiên bản: claude 2.1.300 (Claude Code), tài khoản work `~/.claude`, chạy lồng đã bỏ biến CLAUDE_CODE_*
- Lệnh:
  - `claude -p "nhớ số 42" --output-format json` rồi lấy `.session_id`
  - `claude -p "số tôi bảo nhớ là gì?" --resume <id> --output-format json`
  - băm `sha256sum` và `wc -l` của transcript gốc trước và sau bước fork
  - `claude -p "số đó cộng 1?" --resume <id> --fork-session --output-format json`
  - Điều chỉnh so với nguồn: thêm `--output-format json` ở bước 2 và 3 để lấy được session_id. Chạy lại bằng `bash spikes/s2/run.sh`.
- Kết quả: Đạt (chờ duyệt)
- Quan sát:
  - `--resume` giữ nguyên `session_id` (resume_same_id=yes). `--resume --fork-session` tạo `session_id` mới (fork_new_id=yes) và có file transcript riêng.
  - Transcript gốc không đổi qua bước fork: cùng `sha256` và cùng 65 dòng trước và sau (transcript_unchanged=yes).
  - Câu trả lời: resume nhớ 42; fork trả lời "42 + 1 = 43" (fork_answer_has_43=yes), tức fork kế thừa hội thoại.
  - Cache: request đầu của fork đọc cache của bản gốc. Resume: cache_read 21900 và cache_creation 723; fork: cache_read 22623 và cache_creation 57. 22623 đúng bằng 21900 + 723, tức fork đọc lại toàn bộ tiền tố mà resume vừa ghi (A4 được xác nhận cho CLI `--fork-session`, trong cùng thư mục và vài giây sau resume).
  - Bước 1 mất 3 turn vì Claude tự ghi số 42 vào auto-memory (`~/.claude/projects/<thư mục>/memory/number-42.md`). Do đó phép thử "resume còn nhớ" bị lẫn với bộ nhớ dài hạn; thư mục memory này nằm trong profile work, ngoài repo, không commit.
  - Bucket TTL của lần ghi cache: toàn bộ ở `ephemeral_1h` (10732, 723, 57), `ephemeral_5m` bằng 0.
- Ảnh hưởng tới thiết kế: Phase 4 (OPT/SESS) có thể dùng "golden session" cộng `--resume <id> --fork-session`: mỗi task rẽ một session mới mà không làm đổi session gốc và request đầu đọc được cache của bản gốc (với điều kiện cache của bản gốc còn sống, xem S3). Phase 3 phải lưu `session_id` của phiên gốc và của phiên fork riêng. Phải tính tới auto-memory: task chạy headless có thể ghi file memory làm đổi hành vi giữa các phiên, cần quyết định tắt hay chấp nhận ở Phase 3/4.
- Phương án thay thế: Không cần (kết quả Đạt).
- Phase bị ảnh hưởng: Phase 4, Phase 3
- Bằng chứng:
  - `spikes/s2/raw/s2-1-create.json`
  - `spikes/s2/raw/s2-2-resume.json`
  - `spikes/s2/raw/s2-3-fork.json`
  - `spikes/s2/raw/s2-transcript-hash.txt`
  - `spikes/s2/raw/s2-observations.txt`
  - `spikes/s2/run.sh`

## S3 — Prompt cache

- Ngày thử:
- Phiên bản:
- Lệnh:
- Kết quả:
- Quan sát:
- Ảnh hưởng tới thiết kế:
- Phương án thay thế:
- Phase bị ảnh hưởng:
- Bằng chứng:

## S4 — Kill cây tiến trình

- Ngày thử:
- Phiên bản:
- Lệnh:
- Kết quả:
- Quan sát:
- Ảnh hưởng tới thiết kế:
- Phương án thay thế:
- Phase bị ảnh hưởng:
- Bằng chứng:
- Cách làm đã kiểm chứng (D-13):

## S5 — Tách profile

- Ngày thử:
- Phiên bản:
- Lệnh:
- Kết quả:
- Quan sát:
- Ảnh hưởng tới thiết kế:
- Phương án thay thế:
- Phase bị ảnh hưởng:
- Bằng chứng:

## S6 — DeepSeek qua harness Docker

- Ngày thử:
- Phiên bản:
- Lệnh:
- Kết quả:
- Quan sát:
- Ảnh hưởng tới thiết kế:
- Phương án thay thế:
- Phase bị ảnh hưởng:
- Bằng chứng:

## S7 — Antigravity headless

- Ngày thử:
- Phiên bản:
- Lệnh:
- Kết quả:
- Quan sát:
- Ảnh hưởng tới thiết kế:
- Phương án thay thế:
- Phase bị ảnh hưởng:
- Bằng chứng:

## S8 — Max turns

- Ngày thử:
- Phiên bản:
- Lệnh:
- Kết quả:
- Quan sát:
- Ảnh hưởng tới thiết kế:
- Phương án thay thế:
- Phase bị ảnh hưởng:
- Bằng chứng:

## SPK-10 — Quyết định profile Work/Personal

| Profile | Thư mục | organizationType | seatTier | organizationRole |
|---------|---------|------------------|----------|------------------|
| Work | `~/.claude` | (chờ S5) | (chờ S5) | (chờ S5) |
| Personal | `~/.claude-personal` | (chờ S5) | (chờ S5) | (chờ S5) |

- Câu hỏi chính sách: Chính sách công ty cho phép dùng chỗ ngồi Team của công ty cho việc nào, và tài khoản cá nhân có được dùng cho việc cá nhân/dựng video không?
- Quyết định: (chờ người dùng xác nhận chính sách)
- Người xác nhận / ngày:
