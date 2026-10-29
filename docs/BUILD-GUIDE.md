# Build Guide — agent-brain

Hướng dẫn build, cài và chạy lại `agent-brain` sau khi sửa code. Mọi lệnh chạy từ gốc repo `/mnt/data/IDE/QuangVy-Orchestration`, trừ khi ghi khác.

## 1. Yêu cầu
- Rust toolchain có target `x86_64-unknown-linux-musl`:
  ```bash
  rustup target add x86_64-unknown-linux-musl
  ```
- `musl-gcc` (Route A). Nếu thiếu, script tự dùng `gcc` kèm cờ CFLAGS (Route B) và in cảnh báo.
- Build chạy `--offline --locked`: các crate phải có sẵn trong cache cargo, `Cargo.lock` không được đổi.

## 2. Build nhanh để thử (debug)
Dùng khi chỉ cần biên dịch và chạy test, không cài.
```bash
cargo build --manifest-path agent-brain/Cargo.toml --offline --locked
cd agent-brain
cargo test --lib
cargo test --test cli_lead --test dashboard_pipeline --test autopilot_driver
cargo clippy --all-targets
```

## 3. Build release (bản cài)
```bash
bash scripts/build-release.sh
```
Script làm các bước:
1. Kiểm tra target musl.
2. Chọn trình biên dịch (Route A hoặc B).
3. `cargo build --release --offline --locked --target x86_64-unknown-linux-musl`.
4. Kiểm tra binary liên kết tĩnh (`file`, `readelf`).
5. Chạy thử binary trong thư mục tạm cô lập, gồm cả `serve`.

Binary tạo ra: `agent-brain/target/x86_64-unknown-linux-musl/release/agent-brain`.

## 4. Cài và khởi động lại
1. **Dừng `agent-brain serve`** đang chạy: Ctrl+C trong terminal của nó. Kiểm tra bằng `pgrep -af "agent-brain serve"`. Đợi các run / autopilot đang chạy xong hoặc dừng chúng, để không còn tiến trình nào mở `brain.db`.
2. Cài binary mới:
   ```bash
   install -m 755 agent-brain/target/x86_64-unknown-linux-musl/release/agent-brain ~/.local/bin/agent-brain
   ```
3. Chạy lại:
   ```bash
   agent-brain serve
   ```
4. Kiểm tra: `ls -l ~/.local/bin/agent-brain` phải có thời gian build mới.

## 5. Khi nào cần sao lưu database
Chỉ khi thay đổi làm **nâng lược đồ** `brain.db` (README mục nâng cấp lược đồ: sao lưu `brain.db` trong `~/.local/share/agent-brain`, thử nâng trên bản sao, rồi mới cài). Sửa code thông thường (như fix agy Lead rỗng) không đổi lược đồ nên không cần. Nếu `serve` báo lỗi lược đồ, dừng lại và xem README.

## 6. Quay lại bản cũ
Giữ bản cũ trước khi cài:
```bash
cp -p ~/.local/bin/agent-brain ~/.local/bin/agent-brain.prev
```
Cần quay lại: dừng `serve`, rồi `cp -p ~/.local/bin/agent-brain.prev ~/.local/bin/agent-brain`. Nếu lược đồ DB đã nâng, phải khôi phục cùng lúc cả bản sao lưu DB (xem README).

## 7. Xử lý sự cố
| Triệu chứng | Cách xử lý |
|---|---|
| `Thiếu target x86_64-unknown-linux-musl` | `rustup target add x86_64-unknown-linux-musl` |
| `no matching package` / lỗi offline | Crate chưa có trong cache; chạy `cargo fetch` khi có mạng, rồi build lại |
| `the lock file needs to be updated` | `Cargo.lock` lệch với `Cargo.toml`; không build `--locked` được, kiểm tra thay đổi dependency |
| `Lỗi: binary không tĩnh` | Máy thiếu `musl-gcc` và Route B thất bại; cài `musl-tools` |
| `install: Text file busy` | `serve` hoặc run vẫn đang chạy binary; dừng hết rồi cài lại |
| Dashboard vẫn lỗi như cũ | Chưa khởi động lại `serve` sau khi cài; kiểm `pgrep -af agent-brain` |
