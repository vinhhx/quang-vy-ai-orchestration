# E-09 — bằng chứng cho từng endpoint `anthropic` (D-34, D-40)

Chỉ dùng claude CLI qua `ANTHROPIC_BASE_URL` (không dùng container dsh). Script thăm dò: `spikes/e09/run.sh` và `spikes/e09/verify.sh`, tự kiểm tra offline bằng `E09_OFFLINE=1 bash spikes/e09/run.sh` (in `E09 offline OK`).

D-34 / D-40: E-09 là bằng chứng theo từng endpoint và từng model, không phải cổng chặn. Thiếu bằng chứng thì endpoint chỉ ở trạng thái `chưa kiểm`; không có gì bị chặn.

## Lịch sử: cổng D-02 (trước D-34)

D-34 / D-40 thay thế phần này (bằng chứng theo từng endpoint, không bao giờ là cổng chặn). Giữ nguyên làm lịch sử.

Ngày: 2026-10-27

E-09 (DeepSeek, một endpoint duy nhất) chưa được chạy thật. Người dùng trả lời `skip-e09` ở checkpoint Task 2 vào ngày 2026-10-27: không có lệnh `claude -p` nào gọi tới endpoint DeepSeek, không có dòng `a.*` / `b.*` / `c.*` nào để ghi lại, không có file trong `spikes/e09/raw/` và không có fixture `tests/fixtures/stream/deepseek_e09_*.jsonl`.

Đây không phải một lần thất bại đo được (DeepSeek chưa được chứng minh là hỏng); đây là gate chưa chạy, nên theo quy ước cũ (`skip-e09` thì ghi `fail`) quyết định D-02 được ghi là `fail`. Quyết định này đã bị D-34 / D-40 thay thế: `fail` cũ chỉ còn là lịch sử, không chặn kế hoạch nào.

## Cách ghi

Chạy (người dùng chạy trong shell của mình, key chỉ `export` trong shell đó; xem phần đầu `spikes/e09/run.sh`):

- Cổng nội bộ trước (D-34): `E09_ROUTE_CHECK=1 E09_KEY_ENV=VCADM_GATEWAY_KEY E09_NAME=vcadm-deepseek E09_BASE_URL=https://ai-gateway.vcadm.vn/proxyllm E09_MODELS="deepseek/deepseek-chat deepseek/deepseek-reasoner" bash spikes/e09/run.sh`
- Tuỳ chọn, endpoint chính thức của DeepSeek: `bash spikes/e09/run.sh` (`E09_NAME=deepseek`, `E09_KEY_ENV=DEEPSEEK_API_KEY` mặc định).

Quy tắc `pass` (theo từng endpoint VÀ từng model): với mọi model mà endpoint liệt kê cho một vai trò đang dùng, các dòng sau xuất hiện (probe `a`; nếu `a` lỗi và `a2` chạy lại thì các dòng `a2.*` tương ứng) và probe B (chỉ ở `m1`) cho `b.result_is_error=yes b.api_error_status=401`:

`a.stream_json_parses=yes a.tool_use_write=yes a.result_seen=yes a.result_is_error=no a.usage_input_gt0=yes a.usage_output_gt0=yes a.file_ok=yes a.key_absent=yes`

Giá trị cho phép của `Quyết định:` là `pass`, `fail` hoặc `not-run`. `not-run` là kết quả hợp lệ (D-40): thiếu bằng chứng không chặn gì. Tín hiệu tin cậy của binary là chuỗi `verified` mà người dùng tự thêm vào mục engine của mình trong `config.json`; sau khi `pass`, người dùng thêm `"verified": "<ngày> E-09"` vào mục đó (Claude không sửa `~/.config/agent-brain/config.json`).

Bằng chứng thô nằm ở `spikes/e09/raw/{name}/m{k}/`, fixture ở `tests/fixtures/stream/anthropic_e09_{name}_ok.jsonl`, `anthropic_e09_{name}_m{k}_ok.jsonl` (k ≥ 2) và `anthropic_e09_{name}_auth_error.jsonl`; endpoint này không bao giờ ghi đè bằng chứng của endpoint khác.

## Endpoint: vcadm-deepseek

Quyết định: not-run
Ngày:
Model:
Dòng quan sát:

## Endpoint: deepseek

Quyết định: not-run
Ngày:
Model:
Dòng quan sát:
