# S6b — dsh kill wrapper (D-35)

Container `dsh-deepseek-harness`, người dùng tự chạy trong terminal của mình (agent không chạy). Container phải đang chạy (`docker ps` thấy `Up`); script không bao giờ khởi động, dừng hay khởi động lại container, không dùng `docker inspect`, không liệt kê môi trường, không gửi tín hiệu theo tên tiến trình.

## Cách chạy

```bash
bash spikes/s6/run.sh --kill-wrapper   # bắt buộc trước khi tin kế hoạch 06-23 cho việc thật
bash spikes/s6/run.sh --uid-probe      # tuỳ chọn (giả định A19)
```

`--kill-wrapper` chạy đúng đoạn bọc mà agent-brain sẽ dùng (kế hoạch 06-23), trừ marker:

```text
docker exec -w <scratch> dsh-deepseek-harness sh -c ': ab-dsh-<12 hex>; dsh --profile "$1" "$2"; rc=$?; exit $rc' sh headless <prompt>
```

Prompt yêu cầu dsh chạy `sleep 120`. Script chờ tối đa 120 giây để thấy marker và con `sleep` trong container, ghi lại cây con theo marker, rồi dừng nó theo cách agent-brain sẽ làm: `kill -TERM` tới các PID đã ghi, chờ tối đa 5 giây, `kill -KILL` các tiến trình còn sống (khớp lại theo PID và đầu dòng lệnh), chụp lần cuối. Kết quả nằm ở `spikes/s6/raw/s6-kill-wrapper.txt`.

| Dòng | Ý nghĩa |
|---|---|
| `marker_visible=yes\|no` | `ps` trong container có thấy marker `ab-dsh-…` trên một dòng lệnh không (giả định A18) |
| `sh_parent_kept=yes\|no` | tiến trình `sh` mang marker còn là cha của tiến trình `dsh` suốt lượt chạy (không bị `exec` thay thế) |
| `before:` | cây con theo marker trước khi dừng |
| `leftover:` | tiến trình còn sống sau TERM và tối đa 5 giây chờ |
| `sweep:` | PID bị KILL ở bước quét (`none` nếu không có) |
| `final:` | `none` khi không còn tiến trình nào của lượt này |

`--uid-probe` chạy tác vụ headless cơ bản (thêm một dòng vào `a.txt`) bằng `docker exec -u <uid>:<gid>` của người dùng host, ghi `spikes/s6/raw/s6-uid-probe.txt` với `uid_run_exit=`, `owner uid:gid=`, `contains_edited_by_dsh=`. Chỉ quan sát (giả định A19: `/root/.dsh` có thể không đọc được); không có gì phụ thuộc vào kết quả.

Cách ghi: sau khi chạy, dán nội dung các file raw vào cuộc trò chuyện (không có key trong đó); mỗi kết quả dưới đây là `ok`, `not-ok` hoặc `not-run` (D-40: thiếu bằng chứng không chặn gì). `ok` = `marker_visible=yes`, `sh_parent_kept=yes` và `final: none`. `not-ok` phải được báo lại cho người lập kế hoạch trước khi tin thiết kế của 06-23 cho việc thật (06-19 P6-09 là lần kiểm thật kế tiếp).

## Kết quả kill-wrapper

Kết quả: ok (2026-10-28, `spikes/s6/raw/s6-kill-wrapper.txt`): `marker_visible=yes`, `sh_parent_kept=yes`, `sleep_child_seen=yes`, `leftover: none`, `final: none`. Giả định A18 đúng trên container thật.

## Kết quả uid-probe

Kết quả: not-run
