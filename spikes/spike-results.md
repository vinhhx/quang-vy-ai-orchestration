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

- Ngày thử: 2026-10-15 (mốc 0 lúc 01:30Z; các mốc 1/10/65 phút chạy tách rời, dự kiến xong khoảng 02:36Z)
- Phiên bản: claude 2.1.300 (Claude Code), tài khoản work `~/.claude` (gói Team seat), `--output-format json`, cùng cờ ở mọi lần chạy
- Lệnh:
  - Ba thư mục không phải git ngoài repo `~/.cache/qv-spikes/s3/w1`, `w10`, `w65`; mỗi thư mục có `CLAUDE.md` bắt đầu bằng một nonce duy nhất và khoảng 49 KB chữ giả ngẫu nhiên (phần riêng của dự án, D-09).
  - Mốc 0 (nguội): trong mỗi thư mục chạy `claude -p "Reply with the single word OK" --output-format json </dev/null`.
  - Mốc 1, 10, 65 phút: đúng một lần thăm dò giống hệt trong w1, w10, w65 tính từ lúc kết thúc mốc 0 của chính thư mục đó (ba tiền tố độc lập vì mỗi lần cache hit đều làm lại bộ đếm TTL). Lịch chạy tách rời bằng `setsid nohup bash spikes/s3/schedule.sh`.
  - Chạy lại bằng `bash spikes/s3/run.sh`, phân loại bằng `bash spikes/s3/verify.sh`.
- Kết quả: đang đo — hoàn tất ở Task 8 (chờ duyệt)
- Quan sát:
  - Kiểm tra trước: không có biến FORCE_PROMPT_CACHING_5M, CLAUDE_CODE_PROMPT_CACHE_TTL, ENABLE_PROMPT_CACHING_1H, DISABLE_PROMPT_CACHING, và không có `promptCacheTtl` trong `~/.claude/settings.json`.
  - Mốc 0 (nguội), phần riêng của dự án P = `cache_creation_input_tokens`: w1 = 33733, w10 = 33806, w65 = 33773; `cache_read_input_tokens` của cả ba là 11168 (phần tĩnh dùng chung của Claude Code). Cả ba đều qua ngưỡng 8000 token nên lịch thăm dò đã được khởi động.
  - Bucket TTL của lần ghi cache ở mốc 0: toàn bộ nằm ở `ephemeral_1h` (33733, 33806, 33773), `ephemeral_5m` bằng 0. Nghĩa là gói subscription xin TTL 1 giờ (xác nhận cũng thấy ở S1 và S2); việc máy chủ có giữ đúng 1 giờ hay không chỉ biết được sau khi có kết quả mốc 10 và 65 phút.
  - Ảnh hưởng tới quota subscription: chưa đo được. Số liệu sử dụng lưu trong `~/.claude.json` đã cũ hơn 15 giờ và chỉ có độ phân giải 1 phần trăm, không đủ để thấy tác động của 6 lần chạy nhỏ.
  - Cảnh báo: dòng `rate_limit_event` ở S1 báo mức dùng 7 ngày của tài khoản work là 0.94; nếu vượt hạn mức, tài liệu nói hội thoại chính bị hạ về TTL 5 phút và làm sai lệch phép đo.
- Ảnh hưởng tới thiết kế: (sẽ ghi ở Task 8) kết luận về cửa sổ "golden session" của Phase 4 phụ thuộc vào việc mốc 10 và 65 phút là ấm hay nguội.
- Phương án thay thế: (sẽ ghi ở Task 8 nếu kết quả là Một phần hoặc Không đạt)
- Phase bị ảnh hưởng: Phase 4 (OPT/SESS)
- Bằng chứng:
  - `spikes/s3/raw/s3-precheck.txt`
  - `spikes/s3/raw/s3-timeline.tsv`
  - `spikes/s3/raw/schedule.log`
  - `spikes/s3/raw/s3-w1-t0-*.json`
  - `spikes/s3/raw/s3-w10-t0-*.json`
  - `spikes/s3/raw/s3-w65-t0-*.json`
  - `spikes/s3/run.sh`

## S4 — Kill cây tiến trình

- Ngày thử: 2026-10-15 (01:40Z)
- Phiên bản: claude 2.1.300 (Claude Code), Linux 7.0.0-38-generic (D-08), chạy từ terminal thường của người dùng (nested=no), tài khoản work `~/.claude`
- Lệnh:
  - `bash spikes/s4/run.sh` (người dùng chạy; agent không được chạy lệnh kill). Mỗi lần chạy: `timeout 600 claude -p "Run this exact bash command and wait for it: sleep 31N" --allowedTools "Bash(sleep *)" --output-format stream-json --verbose </dev/null`, N = 7/8/9 (dấu hiệu riêng `sleep 317`, `318`, `319`).
  - r1: `kill -TERM <pid claude>`. r2: `kill -KILL <pid claude>`. r3: khởi chạy qua `setsid`, rồi `kill -TERM -- -<PGID>` (PGID của launcher).
  - Sau mỗi tín hiệu: chờ 5 giây, chụp `ps`, tìm tiến trình sót bằng khớp neo `^sleep 31N$`, rồi quét (sweep) các hậu duệ đã ghi: TERM, 2 giây sau KILL.
  - Điều chỉnh so với nguồn: dùng dấu hiệu duy nhất thay cho `sleep 300` và chụp `ps -o pid,ppid,pgid,sid,etimes,args` chỉ trên các PID đã ghi thay cho `ps -ef | grep sleep`.
- Kết quả: Đạt (chờ duyệt)
- Quan sát:
  - Cây tiến trình trước khi kill (cả 3 lần giống nhau): `timeout` > `claude` > `/bin/bash -c ... eval 'sleep N'` > `sleep N`. Tiến trình Bash con của claude và `sleep` nằm trong session và nhóm tiến trình RIÊNG (sid = pgid = PID của bash con), khác nhóm của `timeout`/`claude`. Vì vậy `kill -- -PGID` của launcher không trực tiếp chạm tới chúng.
  - r1 SIGTERM vào PID claude: launcher thoát mã 143. Sau 5 giây không còn tiến trình nào trong ảnh `after`; không sót `sleep 317` (leftover = none). Claude tự dọn: dòng stream có `system/task_notification` (`status: stopped`) và `task_updated` (`status: killed`), sau đó vẫn in dòng `result` (`subtype: success`, `terminal_reason: completed`, `num_turns: 2`) trước khi thoát. Sweep = none, final = none.
  - r2 SIGKILL vào PID claude: launcher thoát mã 137. Bash con (PID 1087003) và `sleep 318` (PID 1087004) SỐNG SÓT, bị `init`/trình thu dọn tái gán cha (`ppid` đổi thành 5646): leftover = `pid=1087004 ppid=1087003`. Stream bị cắt ngay sau dòng `user` (`tool_result`), không có dòng `result`. Sweep (TERM vào 1087003 và 1087004) dọn sạch: final = none.
  - r3 SIGTERM vào nhóm tiến trình của launcher (PGID 1087367, gồm `timeout` và `claude`): launcher thoát mã 143, leftover = none, sweep = none, final = none. Cũng có `task_notification: stopped` và dòng `result` như r1. Cây con được dọn vì claude nhận SIGTERM rồi tự kill task nền của nó, chứ không phải vì tín hiệu tới thẳng bash con (bash con ở nhóm khác).
  - Lệnh `sleep` được Claude Code chạy dưới dạng task nền (`is_backgrounded: true`, `task_type: local_bash`); trong cả 3 lần, tiến trình bash con vẫn là con trực tiếp của PID claude và nằm trong tập hậu duệ đã ghi. Kết luận phạm vi: chỉ kiểm chứng với lệnh nền do Claude tự chuyển nền, chưa thử một lệnh chạy ở foreground thật sự lâu hoặc một lệnh tự fork tiếp.
  - So với giả định E-05 trong kế hoạch ("SIGTERM tree-kill; SIGKILL và `kill -- -PGID` để sót"): SIGTERM tree-kill đúng; SIGKILL để sót đúng; `kill -TERM -- -PGID` KHÔNG để sót (mâu thuẫn với giả định). SIGKILL vào cả nhóm chưa được thử, nên không rút ra điều gì về nó.
  - Ba tệp `s4-rN.jsonl` là dữ liệu mẫu stream bị cắt cụt/kết thúc sớm cho Phase 3 (r2 không có dòng `result`).
- Ảnh hưởng tới thiết kế: PROC-01/02. Hủy một task phải (1) ghi lại PID claude và toàn bộ hậu duệ (đọc `/proc`/`ps`) TRƯỚC khi gửi tín hiệu, (2) gửi SIGTERM vào PID claude (không gửi SIGKILL trước), (3) chờ vài giây, (4) quét các hậu duệ đã ghi còn sống: TERM, rồi KILL sau 2 giây, kiểm tra bằng thời điểm bắt đầu tiến trình để không đụng PID bị tái sử dụng. Nếu buộc phải SIGKILL claude thì bước quét là bắt buộc vì bash con và `sleep` mồ côi. Parser phải chịu được stream kết thúc không có dòng `result` (bị SIGKILL) và có dòng `result` thành công dù bị SIGTERM (không dùng sự có mặt của `result` để suy ra task tự kết thúc). Theo D-08, nếu sau này chạy trên Windows cần Job Object thay cho cách này.
- Phương án thay thế: Không cần (kết quả Đạt). Nếu sau này `kill -TERM` tới claude không đủ (ví dụ lệnh foreground tự fork), dùng ngay bước quét hậu duệ đã ghi.
- Phase bị ảnh hưởng: Phase 3
- Bằng chứng:
  - `spikes/s4/raw/s4-r1.jsonl`
  - `spikes/s4/raw/s4-r2.jsonl`
  - `spikes/s4/raw/s4-r3.jsonl`
  - `spikes/s4/raw/s4-r1-descendants.txt`
  - `spikes/s4/raw/s4-r2-descendants.txt`
  - `spikes/s4/raw/s4-r3-descendants.txt`
  - `spikes/s4/raw/s4-r1-before.txt`
  - `spikes/s4/raw/s4-r1-after.txt`
  - `spikes/s4/raw/s4-r2-before.txt`
  - `spikes/s4/raw/s4-r2-after.txt`
  - `spikes/s4/raw/s4-r3-before.txt`
  - `spikes/s4/raw/s4-r3-after.txt`
  - `spikes/s4/raw/s4-r1-leftover.txt`
  - `spikes/s4/raw/s4-r2-leftover.txt`
  - `spikes/s4/raw/s4-r3-leftover.txt`
  - `spikes/s4/raw/s4-r1-sweep.txt`
  - `spikes/s4/raw/s4-r2-sweep.txt`
  - `spikes/s4/raw/s4-r3-sweep.txt`
  - `spikes/s4/raw/s4-r1-final.txt`
  - `spikes/s4/raw/s4-r2-final.txt`
  - `spikes/s4/raw/s4-r3-final.txt`
  - `spikes/s4/raw/s4-exit.txt`
  - `spikes/s4/raw/s4-env.txt`
  - `spikes/s4/run.sh`
- Cách làm đã kiểm chứng (D-13): Chuỗi đã để lại `none` ở cả 3 lần: (1) ghi PID claude (con duy nhất của launcher `timeout`) và toàn bộ hậu duệ bằng `/proc`/`ps` kèm thời điểm bắt đầu; (2) `kill -TERM <pid claude>`; (3) chờ 5 giây; (4) quét: mọi hậu duệ đã ghi còn sống (cùng thời điểm bắt đầu) nhận TERM, 2 giây sau nhận KILL; (5) kiểm tra không còn tiến trình nào khớp dấu hiệu. Với SIGTERM bước (4) không có gì để quét; với SIGKILL (r2) bước (4) là thứ duy nhất dọn được bash con và `sleep` mồ côi. Không dùng lệnh kill theo tên tiến trình. Ghi chú sự cố khi chạy: lần chạy đầu của `run.sh` lỗi ở r1 vì dòng `${!REC_PIDS[@]+...}` bị bash 5.2 từ chối (`invalid variable name`); đã sửa thành vòng `for i in "${!REC_PIDS[@]}"` (commit 28dbdf5) và chạy lại từ đầu; script tự xóa output dở dang của lần lỗi.

## S5 — Tách profile

- Ngày thử: 2026-10-15
- Phiên bản: claude 2.1.300 (Claude Code), Linux (D-08). Chế độ `partial` của `spikes/s5/run.sh`: người dùng chọn "Map only, skip live test" nên chỉ chạy phần không cần đăng nhập cá nhân.
- Lệnh:
  - `bash spikes/s5/run.sh clone`: nhân bản `~/.claude-orca` sang `~/.claude-personal` bằng rsync, loại `.credentials.json`, `projects/`, `sessions/`, `history.jsonl`, `file-history/`, `session-env/`, `shell-snapshots/`, `telemetry/`, `backups/`; `chmod 700`.
  - `bash spikes/s5/run.sh partial`: `claude auth status` (profile work với `CLAUDE_CONFIG_DIR` bỏ đặt; bản nhân bản với `CLAUDE_CONFIG_DIR=~/.claude-personal`) qua bộ lọc allowlist; một lệnh `claude -p "hi" --output-format json` ở profile work; cùng lệnh đó với bản nhân bản chưa đăng nhập; đọc `organizationType`/`seatTier`/`organizationRole` từ `oauthAccount` của hai file `.claude.json` (chỉ đọc, qua allowlist); so sánh dấu vân tay `~/.claude-orca` (sha256, kích thước, mtime) trước và sau.
  - Không chạy claude với `~/.claude-orca` và không sửa nó (E-06). Không đăng nhập ở bất kỳ đâu.
- Kết quả: Một phần (chờ duyệt)
- Quan sát:
  - Quyết định của người dùng tại điểm dừng đăng nhập: profile Personal CHÍNH LÀ `~/.claude-orca` (đã đăng nhập sẵn), không phải `~/.claude-personal`. Phiên GSD này đang chạy trên `~/.claude-orca`, nên kế hoạch cấm chạy claude với nó và cấm sửa nó (token có thể xoay vòng, E-06). Vì vậy chỉ ánh xạ, không chạy thử trực tiếp.
  - Bản nhân bản `~/.claude-personal` vẫn nằm đó, KHÔNG dùng và KHÔNG xóa: `credentials_copied=no`, 1003 file, quyền 700, phương pháp rsync.
  - Profile work (`CLAUDE_CONFIG_DIR` bỏ đặt): `claude auth status` báo `loggedIn=true`, `authMethod=claude.ai`, `subscriptionType=team`, `configDirectory=~/.claude`. Bản `auth status` của phiên bản này KHÔNG in `organizationType`/`seatTier`/`organizationRole`; ba trường này lấy từ `oauthAccount` trong `.claude.json`: work = claude_team / team_standard / user; `~/.claude-orca` = claude_pro / (không có seatTier) / admin.
  - Bản nhân bản chưa có credentials: `auth status` báo `loggedIn=false`, `authMethod=none` dù `.claude.json` nhân bản vẫn mang `oauthAccount` của tài khoản orca. Tức là phần nhận dạng trong `.claude.json` không đủ để đăng nhập.
  - `claude -p "hi"` với bản nhân bản chưa đăng nhập: thoát ngay mã 1, `is_error=true`, `result` là "Not logged in · Please run /login", chi phí 0, không gọi API. Profile trống KHÔNG âm thầm rơi về profile work `~/.claude`.
  - `claude -p "hi"` ở profile work: `success`, `is_error=false`, 1 turn. Transcript của session này nằm ở `~/.claude/projects/<thư mục>/<session_id>.jsonl` (work_sid_in_work_profile=yes) và không có trong bản nhân bản (work_sid_in_clone_profile=no).
  - Dấu vân tay `~/.claude-orca`: `.credentials.json` không đổi (credentials_unchanged=yes). `.claude.json` có đổi (claude_json_unchanged=no): phiên agent này đang chạy trên `~/.claude-orca` nên có thể tự ghi file đó; spike chỉ đọc, không chạy claude với nó (claude_run_against_orca=no).
  - CHƯA kiểm chứng: (1) chạy claude thật với profile cá nhân; (2) tách transcript giữa hai profile cho session của profile cá nhân (hai dòng `personal_*` ghi `not_run`); (3) cross-resume hai chiều giữa hai profile (`not_run`); (4) E-06, việc sao chép credentials làm đăng xuất nguồn (không sao chép credentials nên không thử); (5) E-07, `CLAUDE_CONFIG_DIR=~/.claude` tường minh (không thử).
- Ảnh hưởng tới thiết kế: Quy tắc ghi lại cho PROF: bỏ đặt `CLAUDE_CONFIG_DIR` với profile work, chỉ đặt nó cho profile không mặc định (E-07). Profile Personal là `~/.claude-orca`, nên Phase 3 spawn task cá nhân với `CLAUDE_CONFIG_DIR=~/.claude-orca`; profile trống thất bại nhanh và rõ ràng chứ không rơi về work, nên có thể dùng "đăng nhập chưa" làm kiểm tra tiền điều kiện. Vì `~/.claude-orca` cũng là profile của chính phiên điều phối đang chạy, việc spawn thêm tiến trình claude dùng chung credentials đó chưa được kiểm chứng về xoay token (E-06).
- Phương án thay thế: (1) Trước khi thiết kế PROF chốt, chạy phép thử trực tiếp một lần: đăng nhập tài khoản cá nhân vào profile phụ (`CLAUDE_CONFIG_DIR=~/.claude-personal claude`, rồi `/login`) và chạy `bash spikes/s5/run.sh test`; bản nhân bản `~/.claude-personal` được giữ lại đúng để dùng cho việc này. (2) Hoặc chạy phép thử trực tiếp với `~/.claude-orca` từ một terminal ngoài phiên GSD khi không còn phiên nào dùng nó. (3) Trong lúc chưa kiểm: Phase 3 coi thư mục profile là cấu hình, không giả định transcript tách biệt và không chạy hai tiến trình claude song song trên cùng một profile cá nhân.
- Phase bị ảnh hưởng: Phase 2 và Phase 3 (PROF: chọn profile khi spawn, cách ly transcript và credentials)
- Bằng chứng:
  - `spikes/s5/raw/s5-clone.txt`
  - `spikes/s5/raw/s5-auth-work.json`
  - `spikes/s5/raw/s5-auth-personal.json`
  - `spikes/s5/raw/s5-account-facts.txt`
  - `spikes/s5/raw/s5-hi-work.json`
  - `spikes/s5/raw/s5-hi-clone.txt`
  - `spikes/s5/raw/s5-transcripts.txt`
  - `spikes/s5/raw/s5-cross-resume.txt`
  - `spikes/s5/raw/s5-orca-untouched.txt`
  - `spikes/s5/run.sh`

## S6 — DeepSeek qua harness Docker

- Ngày thử: 2026-10-15
- Phiên bản: Docker 29.8.1; container `dsh-deepseek-harness` (không restart, không stop, không rebuild; không liệt kê env, không đọc `.env.docker`); `dsh --profile headless` (bản `--help` không in số phiên bản); chạy từ host Linux (D-08)
- Lệnh:
  - Chế độ basic (agent chạy): `timeout 300 docker exec -w <spikes/s6/scratch> dsh-deepseek-harness dsh --profile headless "Append the line 'edited-by-dsh' to a.txt, then reply DONE" </dev/null`; thêm `dsh --profile headless --help`. Thư mục scratch nằm trong repo (git-ignore), đường dẫn giống hệt bên trong container nhờ bind mount; sau khi chạy được xóa bằng `docker exec ... rm -rf` có kiểm tra đường dẫn kết thúc bằng `/spikes/s6/scratch`.
  - Chế độ kill: `bash spikes/s6/run.sh --kill`. Mỗi biến thể có dấu hiệu riêng `s6k-<nonce>` trong prompt, yêu cầu dsh chạy `sleep 120`. (a) gửi SIGTERM vào PID của `docker exec` client phía host. (b) khởi chạy qua `sh -c` ghi `$$` vào `/tmp/s6pid-<nonce>` trong container rồi `exec dsh`, sau đó `docker exec kill -TERM <pid đó>`. Kiểm tra sót bằng `ps` trong container lọc theo dấu hiệu cùng hậu duệ; quét theo PID.
  - Điều chỉnh so với nguồn: chạy qua container thay vì `ANTHROPIC_BASE_URL` trên host (D-07); biến thể (c) dựa theo mẫu tên tiến trình bị bỏ vì có thể chạm tới tiến trình headless khác không do spike khởi chạy.
  - Chạy lại bằng `bash spikes/s6/run.sh` và `bash spikes/s6/run.sh --kill`.
- Kết quả: Một phần (chờ duyệt)
- Quan sát:
  - (1) Gọi headless từ host qua `docker exec`: được. Thoát mã 0, stdout đúng một dòng `DONE`, stderr rỗng. `dsh --profile headless --help` mô tả "Answer one task, print the final assistant message, and exit"; chỉ có cờ `-h`, không có cờ chọn định dạng đầu ra, không có giới hạn lượt, không có cờ session.
  - (2) Tương đương stream-json: KHÔNG. `s6-format.txt` ghi `json_lines=0 text_lines=1`: stdout chỉ là thông điệp cuối cùng dạng văn bản, không có sự kiện từng bước, không có `session_id`, không có `usage`, không có `tool_use`. Phase 6 không thể dùng parser stream-json của Phase 3 cho dsh.
  - (3) Công cụ sửa file: hoạt động. `a.txt` từ `hello` thành `hello` + `edited-by-dsh` (`contains_edited_by_dsh=yes`). File thuộc `uid:gid=0:0` (root trong container), trong khi người dùng host là `1000:1000`; thư mục scratch phải xóa bằng `docker exec` chứ `rm` thường trên host không đủ quyền với file do dsh tạo/ghi.
  - (4a) Kill từ ngoài, SIGTERM vào `docker exec` client phía host: client thoát (`client_alive_after_signal=no`) nhưng tiến trình TRONG container SỐNG SÓT nguyên vẹn: `node /usr/local/bin/dsh` (PID 245), `bash -c "sleep 120 && echo ..."` (PID 276, nhóm tiến trình riêng) và `sleep 120` (PID 277) đều còn sau 5 giây, giống hệt ảnh trước khi kill. Quét theo PID đã ghi (TERM 245 276 277, không cần KILL) đưa về `final: none`. Giả định E-08 ("kill client host có thể để sót tiến trình trong container") được xác nhận.
  - (4b) Kill bằng TERM tới PID dsh trong container (qua `docker exec kill`): dsh thoát cùng con `sleep 120` (PID 351), leftover = none, sweep = none, final = none. Client phía host cũng thoát. Mã thoát của launcher ghi nhận là 0 ở cả (a) lẫn (b), không phải 143, nên mã thoát của `timeout`/`docker exec` KHÔNG dùng được để suy ra việc bị kill.
  - Khác biệt cần lưu ý: ở (a) dsh dùng một `bash -c` trung gian ở nhóm tiến trình riêng, ở (b) con `sleep` là con trực tiếp của dsh. Cách dsh khởi chạy lệnh có thể thay đổi giữa các lần chạy, nên thiết kế không được dựa vào hình dạng cây này mà phải ghi hậu duệ trước khi kill như ở S4.
  - Phần kill do điều phối viên (agent orchestrator) chạy thay người dùng theo yêu cầu rõ ràng của người dùng ("You support test for me"), vì lần tự chạy trong terminal của người dùng không tạo ra tệp kết quả nào và không có thông báo lỗi nào được chia sẻ. Đây là điều chỉnh so với kế hoạch ("người dùng chạy"); script vẫn chỉ gửi tín hiệu tới PID client do chính nó khởi chạy và PID trong container mang dấu hiệu `s6k-`. Sau khi chạy, không còn tiến trình `s6k-` hay `sleep 120` trong container (kiểm tra lại trong lúc xác minh), và thư mục scratch đã bị xóa.
  - Quét bí mật: `scan-secrets.sh --container dsh-deepseek-harness` in `CLEAN`; không có giá trị khóa nào trong evidence.
  - Chưa kiểm chứng: chất lượng tác vụ lớn hơn một dòng; biến thể kill khi dsh đang chạy lệnh foreground tự fork; `dsh web` API; tính tương thích khóa DeepSeek với endpoint định dạng Anthropic (E-09, không thử).
- Ảnh hưởng tới thiết kế: Phase 6 (ENG) chỉ có thể coi dsh headless là một "hộp đen một lần": đưa prompt vào, nhận một khối văn bản cuối cùng. Không có log theo thời gian thực, không có session để resume, không có token/cost; tiến độ và kết quả phải suy ra từ `git diff`/trạng thái file. Hủy task dsh KHÔNG được dựa vào việc kill `docker exec` client: phải kill tiến trình bên trong container (ghi PID trong container trước, như biến thể (b), hoặc chạy qua `sh -c` ghi `$$`), sau đó quét các hậu duệ đã ghi (cách của S4). File do dsh ghi thuộc root nên Phase 6 cần tính tới quyền sở hữu khi dọn worktree hoặc commit.
- Phương án thay thế: (1) Nếu cần log có cấu trúc hoặc dùng được parser Phase 3: chạy Claude Code trên host với `ANTHROPIC_BASE_URL` trỏ tới endpoint định dạng Anthropic của DeepSeek (chưa kiểm chứng việc khóa có tương thích, E-09; cần Phase 6 thử). (2) Giữ dsh headless, bỏ qua stream: lấy văn bản cuối cộng `git diff` làm kết quả. (3) Mục nghiên cứu cho Phase 6: API của `dsh web` (có thể cung cấp sự kiện có cấu trúc). Với kill: dùng biến thể (b) kèm quét hậu duệ.
- Phase bị ảnh hưởng: Phase 6
- Bằng chứng:
  - `spikes/s6/raw/s6-stdout.txt`
  - `spikes/s6/raw/s6-stderr.txt`
  - `spikes/s6/raw/s6-dsh-help.txt`
  - `spikes/s6/raw/s6-a-after.txt`
  - `spikes/s6/raw/s6-format.txt`
  - `spikes/s6/raw/s6-kill-a.txt`
  - `spikes/s6/raw/s6-kill-b.txt`
  - `spikes/s6/run.sh`
  - `spikes/s6/verify.sh`

## S7 — Antigravity headless

- Ngày thử: 2026-10-15
- Phiên bản: agy 1.3.5 (`~/.local/bin/agy`); trang docs `antigravity.google/docs/cli/headless` lấy ngày 2026-10-15. Kết luận dựa trên docs cộng chạy local (không phải chỉ docs), xem `spikes/s7/NOTES.md`.
- Lệnh:
  - Mọi lệnh chạy dạng `timeout 10 agy ... </dev/null > <tệp>` từ thư mục tạm ngoài repo: `agy --version`; `agy --help`; `agy -p "Reply with the single word OK" --output-format json`; cùng prompt với `--output-format stream-json`; phép thử bác bỏ E-10 `agy -p x --max-turns 1`.
  - Docs lấy bằng `curl` rồi rút thành văn bản (công cụ WebFetch không có trong phiên thực thi). Không chạy binary khởi chạy IDE, không cài gì.
  - Chạy lại bằng `bash spikes/s7/run.sh`.
- Kết quả: Một phần (chờ duyệt)
- Quan sát:
  - Headless: CÓ. `-p` (bí danh `--print`, `--prompt`) chạy một prompt không tương tác rồi thoát. Trên máy này chạy thẳng bằng thông tin đăng nhập đã lưu, không báo `authentication required`, hoàn tất khoảng 3 đến 4 giây, thoát mã 0, không chạm giới hạn 10 s.
  - JSON: CÓ. `--output-format json` cho một phong bì với `conversation_id`, `status` (`SUCCESS`), `response`, `duration_seconds`, `num_turns`, `usage` (`input_tokens`, `output_tokens`, `thinking_tokens`, `cache_read_tokens`, `total_tokens`). `--output-format stream-json` cho NDJSON với khóa `event` thuộc `init`, `step_update`, `result`; khác Claude ở chỗ không có `type`/`tool_use`/`tool_result` mà dùng `step_update.step_type` (docs nêu `user_input`, `agent_response`, `tool`, `checkpoint`; lần chạy local không dùng công cụ nên chưa thấy `tool`). `usage` có ở `step_update` và `result`.
  - Cờ giới hạn lượt: KHÔNG. Bảng cờ trong docs không có, `agy --help` không liệt kê, và phép thử bác bỏ `agy -p x --max-turns 1` thoát mã 2 với `flags provided but not defined: -max-turns`. `num_turns` chỉ là bộ đếm tích lũy, không phải giới hạn. Giả định E-10 được xác nhận. Cờ duy nhất để chặn là `--print-timeout` (thời gian, không phải số lượt).
  - Lệch giữa docs và bản cài: docs nói `--print-timeout` mặc định 5 phút, nhưng `agy --help` của 1.3.5 ghi mặc định `0s` ("chờ đến khi lượt xong"); chưa đo hành vi thật. `agy --help` và lỗi cờ lạ được in ra stderr, không phải stdout.
  - `total_tokens` chỉ là input cộng output; `cache_read_tokens` tách riêng và có thể lớn hơn tổng.
  - Chưa kiểm chứng: tác vụ có công cụ ghi tệp/chạy lệnh (mặc định `permission_mode` là `request-review`, công cụ cần duyệt bị soft-deny và vẫn thoát mã 0), `--input-format stream-json`, `--conversation`/`--continue`, kill giữa chừng, và trường hợp chưa đăng nhập.
- Ảnh hưởng tới thiết kế: ENG-06 có thể tích hợp agy như một engine headless: spawn `agy -p "<prompt>" --output-format stream-json` (hoặc `json`) với stdout ra tệp và `</dev/null`, đọc `result.status`, `result.response`, `result.usage`. Parser của Phase 3 phải có một nhánh riêng cho agy vì sự kiện khác (khóa `event`, không có `tool_use`). Vì không có giới hạn lượt, giới hạn chi phí/độ dài phải dựa vào `--print-timeout` tường minh cộng việc kill cây tiến trình theo cách đã kiểm chứng ở S4 (ghi hậu duệ trước, SIGTERM, quét), và vào `--dangerously-skip-permissions` hoặc `permissions.allow` để tác vụ có công cụ không bị soft-deny im lặng. Cần đặt `--print-timeout` tường minh vì mặc định docs và bản cài khác nhau.
- Phương án thay thế: Vì không có cờ giới hạn lượt: bound mỗi lần chạy bằng `--print-timeout` (ví dụ `5m`) cộng `timeout` bên ngoài, đếm `step_update` của luồng stream-json làm tín hiệu tiến độ, và kill theo S4 khi vượt ngưỡng; nếu cần giới hạn lượt cứng thì dùng engine khác (Claude `--max-turns`, xem S8). Nếu về sau agy hỏi đăng nhập trong môi trường không terminal thì coi đó là điều kiện tiên quyết: đăng nhập agy tương tác một lần trước khi chạy.
- Phase bị ảnh hưởng: Phase 6
- Bằng chứng:
  - `spikes/s7/raw/s7-version.txt`
  - `spikes/s7/raw/s7-help.txt`
  - `spikes/s7/raw/s7-json.json`
  - `spikes/s7/raw/s7-stream.jsonl`
  - `spikes/s7/raw/s7-maxturns.txt`
  - `spikes/s7/NOTES.md`
  - `spikes/s7/run.sh`

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
| Work | `~/.claude` | claude_team | team_standard | user |
| Personal | `~/.claude-orca` | claude_pro | (không có) | admin |

- Câu hỏi chính sách: Chính sách công ty cho phép dùng chỗ ngồi Team của công ty cho việc nào, và tài khoản cá nhân có được dùng cho việc cá nhân/dựng video không?
- Quyết định: Hai profile (hình dạng option-a). Work = chỗ ngồi Team của công ty, thư mục `~/.claude`, dùng cho việc công ty. Personal = tài khoản Pro cá nhân, thư mục `~/.claude-orca` (không phải `~/.claude-personal`), dùng cho việc cá nhân/dựng video. Chính sách công ty: không xác nhận, người dùng miễn (waived by user). Không có nguồn chính sách nào được ghi lại. Câu trả lời nguyên văn của người dùng ngày 2026-10-15: "SPK-10 alway accept ignore condition". Diễn giải của điều phối viên: người dùng chấp nhận cách ánh xạ hai profile và chủ động chọn tiếp tục mà không xác nhận điều kiện chính sách công ty. Ánh xạ thư mục lấy từ câu trả lời nguyên văn tại điểm dừng đăng nhập S5: "personal logged in change claude config personal is claude-orca". Quan sát tài khoản: `~/.claude` là chỗ ngồi Team của công ty (claude_team), `~/.claude-orca` là tài khoản Pro cá nhân (claude_pro, admin); tiền đề "cả hai tài khoản đều do công ty cấp" không khớp với máy này (E-12).
- Người xác nhận / ngày: Người dùng (vinhhx) / 2026-10-15
