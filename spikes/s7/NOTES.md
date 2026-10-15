# S7 — Antigravity headless (agy): ghi chú nguồn

Kết luận của S7 dựa trên **docs cộng chạy local** (D-11): trang docs chính thức `https://antigravity.google/docs/cli/headless` (lấy ngày 2026-10-15 bằng `curl`, vì công cụ WebFetch không có trong phiên thực thi này; nội dung HTML được rút thành văn bản để đọc, không lưu vào repo) và các lệnh `agy` thật trên máy này (agy 1.3.5), mỗi lệnh có `timeout 10`, `</dev/null`, stdout ghi ra tệp. Không cài gì mới. Chỉ dùng binary `agy`, không chạy trình khởi chạy IDE.

Mỗi dòng bên dưới ghi rõ `Nguồn: docs` hoặc `Nguồn: chạy local`; khi cả hai khớp thì ghi cả hai.

## Cờ headless

- Nguồn: docs. Chạy một prompt rồi thoát bằng `-p`, bí danh `--print` và `--prompt`.
- Nguồn: chạy local. `agy --help` (xem `raw/s7-help.txt`) liệt kê `-p`, `--print`, `--prompt`, `--output-format`, `--input-format`, `--print-timeout`, `--json-schema`, `--continue`/`-c`, `--conversation`, `--dangerously-skip-permissions`, `--mode`, `--model`, `--effort`, `--sandbox`, `--log-file`.

## Định dạng đầu ra

- Nguồn: docs. `--output-format` nhận `text` (mặc định), `json` (một đối tượng JSON in khi xong), `stream-json` (NDJSON).
- Nguồn: chạy local. Cả ba giá trị có trong `--help`. `json` và `stream-json` chạy được thật (`raw/s7-json.json`, `raw/s7-stream.jsonl`), thoát mã 0 trong vài giây, nằm trong giới hạn 10 s.

## Phong bì json

- Nguồn: docs. Trường: `conversation_id`, `status`, `response`, `error` (chỉ khi lỗi), `duration_seconds`, `num_turns`, `structured_output` và `json_schema` (chỉ với `--json-schema`), `usage` (`input_tokens`, `output_tokens`, `thinking_tokens`, `cache_read_tokens`, `total_tokens`). Giá trị `status`: SUCCESS, ERROR, CANCELED, INTERRUPTED, INVALID, WAITING, RUNNING.
- Nguồn: chạy local. Lần chạy thật có đúng các khóa `conversation_id`, `status` (`SUCCESS`), `response` (`OK\n`), `duration_seconds` (khoảng 3.7), `num_turns` (1), `usage`. Khớp docs. Lưu ý `cache_read_tokens` (10424) lớn hơn `total_tokens` (5468), tức `total_tokens` chỉ là input cộng output, không gồm cache đọc.

## Loại sự kiện stream-json

- Nguồn: docs. Luồng bắt đầu bằng một sự kiện `init`, theo sau là các `step_update` (các `step_type` quan sát được gồm `user_input`, `agent_response`, `tool`, `checkpoint`; `state` là `ACTIVE` hoặc `DONE`), và kết thúc bằng một `result` (cùng hình dạng với `json`). Khóa phân loại là `event`, không phải `type`.
- Nguồn: chạy local. Có `init` (kèm `cwd`, danh sách `tools`, `permission_mode: request-review`), 3 `step_update` (`user_input`, `agent_response` ACTIVE với `text_delta` "OK", `agent_response` DONE kèm `usage`) và 1 `result`. Không có `tool_use`/`tool_result` kiểu Claude; đây là lần chạy không dùng công cụ nên chưa thấy `step_type: tool`.

## Giới hạn lượt (turn limit)

- Nguồn: docs. Bảng cờ chính thức (`Flag reference`) không có cờ giới hạn số lượt. `num_turns` chỉ là bộ đếm tích lũy theo phiên, không phải giới hạn. Cờ duy nhất để chặn thời gian là `--print-timeout`.
- Nguồn: chạy local. Phép thử bác bỏ giả định E-10: `agy -p x --max-turns 1` thoát mã 2 với thông báo `flags provided but not defined: -max-turns` (xem `raw/s7-maxturns.txt`). Tức là KHÔNG có cờ `--max-turns`. Giả định "agy không có cờ giới hạn lượt" được xác nhận cả bằng docs lẫn chạy thật.

## Xác thực

- Nguồn: docs. Chế độ headless dùng thông tin đăng nhập đã lưu; cần đăng nhập `agy` tương tác một lần. Khi không có terminal và chưa đăng nhập, lệnh thoát với lỗi `authentication required` thay vì treo.
- Nguồn: chạy local. Trên máy này `agy -p` chạy thẳng bằng thông tin đăng nhập đã lưu, không báo `authentication required`, hoàn tất trong khoảng 3 đến 4 giây (A7 xác nhận trong ngưỡng 10 s). Không thử trường hợp chưa đăng nhập.

## `--print-timeout`

- Nguồn: docs. Mặc định chờ tối đa 5 phút (`--print-timeout 5m`); đặt bằng ví dụ `15m`.
- Nguồn: chạy local. `agy --help` ghi mặc định là `0s` và giải thích "0 waits until the turn completes", khác với docs (5 phút). Chưa đo hành vi thật; bản 1.3.5 của máy này có thể không còn khớp trang docs. Phase 6 nên luôn đặt `--print-timeout` tường minh.

## Quyền công cụ trong headless

- Nguồn: docs. Công cụ cần phê duyệt mà không hỏi được thì bị "soft-denied": lượt chạy vẫn tiếp tục, thoát mã 0 và in thông báo ra stderr. `--dangerously-skip-permissions` phê duyệt mọi lệnh; có thể cấp trước qua `permissions.allow` trong file cài đặt của agy. Mặc định `permission_mode` là `request-review`.
- Nguồn: chạy local. `init` báo `permission_mode: request-review`. Không thử công cụ có ghi tệp hay chạy lệnh.

## Giới hạn của bằng chứng

- Chỉ chạy một prompt một từ; chưa thử tác vụ có công cụ, chưa thử `--input-format stream-json`, chưa thử `--conversation`/`--continue`, chưa thử kill giữa chừng, chưa thử trường hợp chưa đăng nhập.
- Chưa thử đo hành vi `--print-timeout` thật sự.
