# Plan (dự án `srt`): giọng đọc không treo, không lẫn giọng; agent luôn dùng `.venv/bin/python` và không sửa script dùng chung

Repo: `/mnt/data/source_codes/code/projects/affiliates/services/video/srt` (KHÔNG phải repo agent-brain), làm trên một git worktree riêng tách từ `main` của `srt` (ví dụ `git worktree add <tạm>/srt-fix -b fix/tts-timeout main`), vì working tree chính của `srt` đang được agent-brain dùng cho task 12. Đọc toàn bộ file này trước khi làm.

## Sự cố gốc (2026-11-06, task 12 `dung-1-video-1-2-phut-de-huong-dan-cach-giai`, run 63)
1. `scripts/voice_first_pacing.py::synthesize_cue_clips` (~dòng 129–166) gọi `subprocess.run([sys.executable, "-m", "edge_tts", …], check=True, capture_output=True)` KHÔNG có `timeout`. Câu 5 treo khi gọi mạng, chờ vô hạn (từ 10:32 tới khi agy kết thúc lúc 10:45). Cả bước dựng video bị kẹt.
2. Khi edge-tts lỗi 3 lần, script lặng lẽ chuyển sang `gTTS` (giọng Google khác hẳn). Câu 2–3 của task 12 bị đọc bằng gTTS trong khi người dùng chọn `vi-VN-HoaiMyNeural` → video lẫn hai giọng. Cache `cue_XXX.txt` lưu cùng khoá `voice|rate|pitch|text` cho cả hai trường hợp, nên lần chạy sau vẫn dùng lại clip gTTS.
3. Python hệ thống (`python3`) không có `cv2`; chỉ `.venv/bin/python` đủ thư viện. Lead của task 12 lập plan dùng `python3` rồi sửa script dùng chung `scripts/project_config.py` để tự nạp `site-packages` của `.venv` và gắn code vẽ riêng của bài (`scripts/ga_va_cho_assets.py`) vào đó. `AGENTS.md`/`VIDEO_WORKFLOW.md` không nói rõ phải dùng `.venv/bin/python` và không cấm sửa `scripts/`.

## Mục tiêu
1. Mỗi lần gọi edge-tts có `timeout` (hằng `TTS_TIMEOUT_SEC = 60`), thử lại tối đa 3 lần (chờ tăng dần 1s, 3s, 6s). `subprocess.TimeoutExpired` được tính như một lần lỗi. Xoá file mp3 dở trước khi thử lại.
2. Bỏ dự phòng gTTS mặc định. Khi edge-tts vẫn lỗi sau 3 lần: dừng với lỗi rõ ràng `[err] Không tạo được giọng đọc câu N bằng <voice> sau 3 lần (lỗi cuối: …). Kiểm tra mạng rồi chạy lại; các câu đã tạo được giữ trong voice_cues/.` và mã thoát khác 0. Thêm cờ `--allow-gtts-fallback` (mặc định tắt) cho ai muốn dự phòng; khi dùng thì khoá cache ghi thêm `|gtts` để không bị nhầm với clip edge-tts.
3. Cache: chỉ coi là hợp lệ khi có cả `.mp3` (> 0 byte) và `.txt` khớp khoá; mp3 không có `.txt` (dở dang) thì tạo lại.
4. `build_video.py` truyền lỗi này lên (không nuốt), để `--stage all` dừng ngay với lỗi trên.
5. `AGENTS.md` và `VIDEO_WORKFLOW.md` thêm mục "Quy tắc cho agent chạy tự động" (ngắn, đặt gần đầu):
   - Luôn chạy script bằng `.venv/bin/python scripts/<tên>.py …` (Python hệ thống thiếu thư viện). Không sửa `sys.path`/`sys.executable` trong script để né việc này.
   - Không sửa script dùng chung trong `scripts/` khi làm một video. Code hay dữ liệu riêng của một video đặt trong `projects/<tên>/` (ví dụ `projects/<tên>/tools/draw_assets.py`, chạy bằng `.venv/bin/python projects/<tên>/tools/draw_assets.py` nếu được cấp quyền).
   - Khi người dùng đã duyệt plan trong agent-brain (studio) thì coi như đã duyệt Gate 1–4: chạy `--stage all`.
   - Giọng đọc: dùng `--voice` của build_video; nếu tạo giọng lỗi thì dừng và báo, không tự đổi giọng khác.
6. KHÔNG đưa `scripts/project_config.py` hay `scripts/ga_va_cho_assets.py` của nhánh task 12 vào `main` (nhánh đó tự xử lý sau).

## Thay đổi (chỉ các file này, trong worktree `fix/tts-timeout`)
| # | File | Sửa gì |
|---|---|---|
| 1 | `scripts/voice_first_pacing.py` | Mục tiêu 1–3 (+ cờ `--allow-gtts-fallback`) |
| 2 | `scripts/build_video.py` | Mục tiêu 4 (truyền cờ `--allow-gtts-fallback` xuống pacing nếu có; lỗi pacing làm `--stage pacing`/`all` thoát khác 0) |
| 3 | `AGENTS.md`, `VIDEO_WORKFLOW.md` | Mục tiêu 5 |

## Không được đụng tới
`projects/**` (dữ liệu các video), `assets/**`, các script khác trong `scripts/`, `.gsd/**`, nhánh `task/*`, working tree chính của `srt`. Không push. Không `git add -A`, không `--no-verify`.

## Cách kiểm tra (trong worktree, dùng `.venv` của repo chính bằng đường dẫn tuyệt đối nếu worktree không có `.venv`)
```bash
<srt>/.venv/bin/python -m py_compile scripts/voice_first_pacing.py scripts/build_video.py
# Giả lập treo: đặt PATH/biến để edge_tts không tới được mạng là khó; thay vào đó chạy một test nhỏ:
#   monkeypatch subprocess.run trong một script tạm (ngoài repo) để ném TimeoutExpired → hàm phải thử 3 lần rồi ném lỗi rõ ràng, không gọi gTTS.
#   với allow_gtts_fallback=True → gọi gTTS (mock) và khoá cache có '|gtts'.
#   mp3 có mà thiếu .txt → bị tạo lại.
<srt>/.venv/bin/python scripts/build_video.py --help    # có cờ mới
git -C <worktree> diff --stat main..HEAD                # chỉ 4 file
```
Ghi script test tạm và kết quả vào tóm tắt (không commit script test).

## Commit
Trên nhánh `fix/tts-timeout`: một commit cho script (`tts: timeout va thu lai edge-tts, bo du phong gTTS mac dinh`), một commit cho tài liệu (`docs: quy tac agent chay tu dong (.venv, khong sua scripts dung chung)`), theo kiểu message tiếng Việt không dấu của repo, kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. KHÔNG merge vào `main` (Claude review rồi merge khi task 12 xong).
