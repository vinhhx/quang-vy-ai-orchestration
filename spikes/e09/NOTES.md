# E-09 — DeepSeek qua claude (D-02)

Ngày: 2026-10-27

Chỉ dùng claude CLI (không dùng container dsh, D-01). Script thăm dò: `spikes/e09/run.sh` và `spikes/e09/verify.sh`, đã tự kiểm tra offline bằng `E09_OFFLINE=1 bash spikes/e09/run.sh` (in `E09 offline OK`).

## Kết quả

E-09 chưa được chạy thật. Người dùng trả lời `skip-e09` ở checkpoint Task 2 vào ngày 2026-10-27: không có lệnh `claude -p` nào gọi tới endpoint DeepSeek, không có dòng `a.*` / `b.*` / `c.*` nào để ghi lại, không có file trong `spikes/e09/raw/` và không có fixture `tests/fixtures/stream/deepseek_e09_*.jsonl`.

Đây không phải một lần thất bại đo được (DeepSeek chưa được chứng minh là hỏng); đây là gate chưa chạy, nên theo quy ước của kế hoạch (`skip-e09` thì Task 3 ghi `fail`) quyết định được ghi là `fail`.

## Quyết định

`fail`

- E-09 bị người dùng bỏ qua (không chạy, không phải lỗi đo được).
- Hệ quả (D-02): kế hoạch 06-11 (engine DeepSeek) không được tiến hành cho tới khi E-09 được chạy thật (`bash spikes/e09/run.sh` với `DEEPSEEK_API_KEY` export trong shell của người dùng, rồi ghi lại quyết định `pass-alias` / `pass-explicit`), hoặc người dùng quyết định dsh (hộp đen) hay bỏ DeepSeek.
- agy, teams, quota, Settings và engine human không phụ thuộc kết quả này và vẫn tiến hành.
- Cổng pha của ENG-01 (một task `tiet-kiem` có Dev chạy trên DeepSeek) chưa thể đạt khi chưa có E-09.
