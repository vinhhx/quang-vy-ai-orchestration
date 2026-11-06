# Plan: task "làm ra sản phẩm" (ví dụ dựng video) phải chạy tới sản phẩm thật, QA không được cho qua khi thiếu sản phẩm, và agent-brain gửi sản phẩm qua Telegram

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm.
Làm SAU plan `.planning/quick/agy-finish-after-result-and-command-audit.plan.md` (cùng đụng `roles/mod.rs`; rebase lên kết quả của plan đó).

## Sự cố gốc (đã kiểm tra, 2026-11-06, dự án `srt`, task 11, workflow `generate-video`, đội `antigravity-full`)
Yêu cầu: "Tạo video phóng sự … Độ dài 5 phút". Task kết thúc `qa_pass`, nhưng KHÔNG có video (`projects/chien-tranh-nga-ukraina/final/` không tồn tại).
1. **Plan đặt các lệnh dựng video ở `## Cách kiểm tra`**, không phải việc của Dev: `build_video.py --stage pacing|render|assemble` chỉ có ở mục 3 Cách kiểm tra. `assets/prompts/dev.md` và `dev-fix.md` bảo Dev "Không tự ý chạy test hay lệnh kiểm tra; brain sẽ in phần Cách kiểm tra để người dùng tự chạy" → Dev không bao giờ chạy render.
2. **QA cho qua dù thiếu sản phẩm.** QA vòng 1 (run 57) báo đúng "video chưa từng được render… thư mục final/ chưa tồn tại" → `Cần sửa`. Dev sửa hai lượt chỉ bằng cách viết lại tài liệu ("chờ Gate", "người dùng tự chạy"). QA vòng 3 (run 61) kết luận `Đạt`. Không có kiểm tra tất định nào về sản phẩm.
3. **Cổng duyệt của dự án mâu thuẫn với workflow.** `srt/AGENTS.md`, `VIDEO_WORKFLOW.md` bắt dừng ở Gate 1–4; workflow chỉ có một `gate:you` (duyệt plan). Agent coi cổng nội bộ của dự án là lý do dừng.
4. **Không gửi được video qua Telegram.** `telegram/summary.rs::report_documents` chỉ đọc file đã commit trên nhánh task, ≤ `MAX_REPORT_FILE_SIZE` (1 MB), nội dung chuyển thành `String` (`Body::Upload { content: String }` trong `telegram/transport.rs`) → không gửi được file nhị phân. `*.mp4` lại bị `.gitignore` của `srt`.

## Mục tiêu
1. Plan có hai mục tuỳ chọn mới:
   - `## Tạo sản phẩm`: danh sách có thứ tự các lệnh Dev PHẢI chạy sau khi sửa file (mỗi dòng một lệnh trong backtick). Mỗi lệnh phải khớp một mẫu trong `## Lệnh được chạy`.
   - `## Sản phẩm`: các file kết quả (mỗi dòng một file trong backtick, tối đa 5), ví dụ `` - `projects/x/final/x-complete.mp4` ``. File có thể bị gitignore.
2. Dev (implement và fix) chạy đúng các lệnh `## Tạo sản phẩm` theo thứ tự. Prompt Dev không còn cấm chạy những lệnh này.
3. Sau mỗi lượt QA, agent-brain kiểm TẤT ĐỊNH từng file `## Sản phẩm`: tồn tại, là file thường, nằm trong gốc dự án (sau khi resolve symlink), kích thước > 0. Thiếu file → kết luận QA bị đổi thành `Cần sửa` (dù QA ghi `Đạt`), thêm một dòng giải thích vào `.qa-report.md`, đi vào vòng fix như thường.
4. Khi task qua QA, sau file Tóm tắt và các file `## Gửi báo cáo`, bot gửi từng file `## Sản phẩm` đọc từ cây làm việc:
   - `.mp4`/`.mov`/`.webm` → `sendVideo` (`supports_streaming=true`); loại khác → `sendDocument`;
   - file > `TELEGRAM_UPLOAD_MAX_BYTES` (50 MB, giới hạn Bot API) → không upload, gửi tin nhắn văn bản: tên file, kích thước MB, đường dẫn tuyệt đối trên máy.
5. Prompt Lead: khi yêu cầu là làm ra sản phẩm (video, file xuất…), plan phải đi tới sản phẩm cuối, đưa lệnh tạo vào `## Tạo sản phẩm`, khai `## Sản phẩm`; việc người dùng duyệt plan ở `gate:you` được coi là đã duyệt mọi cổng nội bộ của dự án (Gate 1–4 của `srt`).

## Thay đổi (chỉ các file này)

| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/planfile.rs` | Mục A |
| 2 | `agent-brain/src/roles/dev.rs` | Mục B (đưa lệnh `## Tạo sản phẩm` vào tin nhắn đầu vào của Dev) |
| 3 | `agent-brain/src/roles/qa.rs` | Mục C |
| 4 | `agent-brain/src/telegram/transport.rs` | Mục D1 |
| 5 | `agent-brain/src/telegram/summary.rs` | Mục D2 |
| 6 | `agent-brain/src/telegram/outbox.rs` | Mục D3 |
| 7 | `agent-brain/assets/prompts/lead.md` | Mục E |
| 8 | `agent-brain/assets/prompts/dev.md`, `agent-brain/assets/prompts/dev-fix.md` | Mục E |
| 9 | `agent-brain/assets/prompts/qa.md` | Mục E |
| 10 | `brain/prompts/lead.md`, `brain/prompts/dev.md`, `brain/prompts/dev-fix.md`, `brain/prompts/qa.md` | Sinh lại từ #7–#9 đúng cách các plan trước đã làm (xem `.planning/quick/telegram-report-files.plan.md` mục 6 và test `tests/assets_parity.rs`). Không gõ tay. |
| 11 | Test: unit trong `planfile.rs`, `summary.rs`, `transport.rs`; `tests/cli_qa.rs`, `tests/cli_dev.rs`, `tests/telegram_notify.rs`, `tests/assets_parity.rs` (chỉ khi cần cập nhật parity) | Mục Test |

KHÔNG sửa template/skeleton `.plan.md` (có test parity `templates_equal_phase2`); hai mục mới là tuỳ chọn, Lead tự thêm.
Không sửa: `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dự án `srt`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.

### A. `src/planfile.rs`
1. Hằng `pub const PRODUCE_SECTION: &str = "Tạo sản phẩm";`, `pub const PRODUCT_SECTION: &str = "Sản phẩm";`. Cẩn thận: `grant_section` tìm theo tiền tố tên mục; `"Sản phẩm"` không được khớp nhầm `"Tạo sản phẩm"` và ngược lại. Kiểm bằng test.
2. `pub fn produce_commands(plan: &Plan) -> Vec<String>` (giữ thứ tự, không bỏ trùng) và `pub fn product_files(plan: &Plan) -> Vec<String>` (chuẩn hoá bằng `normalize_plan_path`, bỏ trùng, giữ thứ tự), dùng lại cách đọc của `report_files`/`grant_items`.
3. `validate_plan` (lỗi tiếng Việt rõ ràng, đi vào cơ chế Lead tự sửa 1 lần đã có):
   - Mỗi lệnh `## Tạo sản phẩm`: qua `grant_command_error` và khớp ít nhất một mẫu của `## Lệnh được chạy` theo đúng luật khớp mà guard dùng (dùng lại hàm khớp của guard, ví dụ `command_matches`; nếu đang private thì thêm wrapper `pub`).
   - Mỗi file `## Sản phẩm`: tương đối, không `..`, không bắt đầu `/` hay `~`, không khớp `guard::grant_denied_rel`, và nằm trong một thư mục `## Thư mục được ghi` hoặc có trong bảng Thay đổi.
   - Tối đa 5 file sản phẩm, tối đa 20 lệnh tạo sản phẩm.
   - Có `## Sản phẩm` mà không có `## Tạo sản phẩm` → hợp lệ (Dev có thể tạo file bằng việc sửa trong bảng Thay đổi).

### B. `src/roles/dev.rs`
Khi plan có `## Tạo sản phẩm`, thêm vào tin nhắn đầu vào của Dev (implement và fix) một khối:
"Sau khi sửa xong các file, chạy lần lượt các lệnh sau từ gốc dự án, mỗi lệnh riêng một lần, đợi lệnh xong mới chạy lệnh tiếp. Lệnh nào lỗi: sửa nguyên nhân trong phạm vi plan rồi chạy lại; không sửa được thì ghi Vướng mắc và dừng. Không đánh dấu hoàn thành khi chưa có đủ file ở mục Sản phẩm:" + danh sách lệnh + danh sách file sản phẩm.
Không đổi phạm vi quyền: các lệnh đã nằm trong `## Lệnh được chạy`.

### C. `src/roles/qa.rs` — kiểm sản phẩm tất định
Sau khi đọc `verdict` (~dòng 247) và TRƯỚC `commit_paths(.qa-report.md)`:
1. `let missing = check_products(&proj_path, &plan)` — trả danh sách `(file, lý do)` với lý do: "không tồn tại", "không phải file thường", "ra ngoài gốc dự án", "rỗng (0 byte)". Dùng `std::fs::symlink_metadata` + `canonicalize` so với gốc dự án đã canonicalize.
2. Nếu `!missing.is_empty()`:
   - nối vào cuối `.qa-report.md` một đoạn `\n## Kiểm tra sản phẩm (agent-brain)\n- <file>: <lý do>\n…\n` và sửa dòng dưới `## Kết luận` thành `Cần sửa` (viết hàm nhỏ thay đúng dòng kết luận, không đụng phần khác);
   - `verdict = "Cần sửa"`; in `"Thiếu sản phẩm — QA chuyển thành Cần sửa"`.
3. Nếu đủ: nối `\n## Kiểm tra sản phẩm (agent-brain)\n- <file>: có, <kích thước> byte\n`. Giữ nguyên kết luận QA.
4. Phần còn lại (commit, outcome, status) giữ nguyên, nên `qa_fix` đi vào vòng fix như thường.

### D. Telegram gửi sản phẩm
1. `transport.rs`: thêm biến thể `Body::UploadFile { fields: Vec<(String, String)>, file_field: String, path: PathBuf, filename: String }`. Curl dùng `-F <field>=@<path>;filename=<safe_form_word(filename)>`. Đường dẫn đi qua file `-K` config như token (không lộ trên argv), escape đúng như các giá trị khác trong config. Thời gian tối đa của lệnh gọi do người gọi truyền.
2. `summary.rs`: `pub fn product_files_for_send(s: &TaskState) -> Vec<ProductSend>` với `ProductSend { path: PathBuf, filename: String, size: u64, kind: Video | Document | TooLarge }`, đọc từ cây làm việc ở `s.root`, chỉ các file `planfile::product_files`, áp cùng các kiểm tra an toàn như Mục C (trong gốc dự án sau canonicalize, file thường, không `grant_denied_rel`). File thiếu bỏ qua (QA đã chặn). Hằng `pub const TELEGRAM_UPLOAD_MAX_BYTES: u64 = 50 * 1024 * 1024;`.
3. `outbox.rs` (vòng `EventKind::QaPass`, sau phần gửi report_docs ~dòng 370–410, trước tin nhắn văn bản): gửi lần lượt từng sản phẩm với cùng cơ chế retry/`continue`/kiểm `current_event` như report docs:
   - Video → `sendVideo` field `video`, thêm field `supports_streaming=true`; Document → `sendDocument` field `document`; caption `"Sản phẩm task {id}: {filename}"` qua `render::clip`.
   - TooLarge → `sendMessage` (JSON) với văn bản: `"Sản phẩm task {id}: {filename} ({x.y} MB) lớn hơn 50 MB nên không gửi được qua Telegram. File ở: {đường dẫn tuyệt đối}"`.
   - Thời gian tối đa mỗi lần upload: hằng `PRODUCT_UPLOAD_MAX_SECS: u64 = 600`.
   - Lỗi không retry được → in một dòng như report docs rồi bỏ qua file đó, không chặn tin nhắn cuối.

### E. Prompt
1. `lead.md`, thêm sau dòng về `## Gửi báo cáo`:
   - "Khi yêu cầu là làm ra sản phẩm (ví dụ dựng video, xuất file): plan phải đi tới sản phẩm cuối cùng trong một task. Đưa các lệnh tạo sản phẩm vào mục `## Tạo sản phẩm` (mỗi dòng một lệnh trong backtick, đúng thứ tự, mỗi lệnh phải có trong `## Lệnh được chạy`), và liệt kê file kết quả trong mục `## Sản phẩm` (tối đa 5 file). agent-brain kiểm các file này sau QA và tự gửi cho người dùng qua Telegram. KHÔNG đặt lệnh tạo sản phẩm chỉ ở `## Cách kiểm tra`."
   - "Người dùng đã duyệt plan của bạn ở bước duyệt của agent-brain; coi đó là đã duyệt trước mọi cổng duyệt nội bộ của dự án (ví dụ Gate 1–4 trong AGENTS.md/VIDEO_WORKFLOW.md). Không lập plan dừng ở cổng nội bộ; dùng lệnh chạy hết các bước (ví dụ `--stage all`) khi dự án có."
2. `dev.md` và `dev-fix.md`: thay dòng "Không tự ý chạy test hay lệnh kiểm tra; …" bằng: "Không tự ý chạy test hay lệnh kiểm tra, TRỪ các lệnh ở mục Tạo sản phẩm trong tin nhắn đầu vào: các lệnh đó bắt buộc chạy. Không được báo xong hay ghi tài liệu nói sản phẩm đã có khi file sản phẩm chưa tồn tại."
3. `qa.md`: thêm "Nếu plan có mục Sản phẩm: kiểm từng file thật sự tồn tại (dùng công cụ đọc/liệt kê file); tài liệu tự khai 'đã xong' không phải bằng chứng. Thiếu sản phẩm hoặc lệnh trong Cách kiểm tra lỗi → Cần sửa. 'Người dùng tự chạy' không phải lý do để Đạt."

## Không được đụng tới
- Logic gửi Tóm tắt và `## Gửi báo cáo` hiện có (chỉ thêm bước sau đó).
- Quy tắc grants/guard hiện có (chỉ thêm hàm `pub` wrapper nếu cần).
- Không `git add -A`, không `--no-verify`.

## Test (bắt buộc), dữ liệu giả trong thư mục tạm, không đọc dự án `srt` thật
- (a) `planfile`: đọc `## Tạo sản phẩm`/`## Sản phẩm` đúng, không lẫn nhau; validate: lệnh không có trong `## Lệnh được chạy` bị từ chối; file ngoài thư mục ghi bị từ chối; `../x`, `/abs`, `.env` bị từ chối; plan giống plan `srt` (lệnh `.venv/bin/python scripts/build_video.py --project projects/x --stage all` với mẫu `.venv/bin/python scripts/build_video.py *`, sản phẩm `projects/x/final/x-complete.mp4`, thư mục ghi `projects/x/**`) hợp lệ.
- (b) `cli_dev`: plan có `## Tạo sản phẩm` → tin nhắn đầu vào của Dev chứa đúng danh sách lệnh và file sản phẩm theo thứ tự.
- (c) `cli_qa`: QA giả ghi `Đạt`, file sản phẩm không tồn tại → task `qa_fix`, `.qa-report.md` có mục "Kiểm tra sản phẩm (agent-brain)" và Kết luận `Cần sửa`; tạo file sản phẩm (gitignored) → `qa_pass`; symlink trỏ ra ngoài gốc → `Cần sửa`; file 0 byte → `Cần sửa`. Plan không có `## Sản phẩm` → hành vi cũ.
- (d) `transport`: `Body::UploadFile` sinh config curl có `@<path>` đã escape, không đưa path lên argv; tên file có `;` được làm sạch.
- (e) `telegram_notify`: QA pass với sản phẩm `.mp4` nhỏ → gọi `sendVideo` với `supports_streaming`; `.pdf` → `sendDocument`; file giả > 50 MB (dùng `File::set_len`, không ghi thật) → `sendMessage` có đường dẫn và kích thước, không upload; thứ tự: Tóm tắt → báo cáo → sản phẩm → tin nhắn.
- (f) Toàn bộ test cũ pass, gồm parity prompt.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
git diff --stat <HEAD trước khi chạy>..HEAD   # chỉ các file trong bảng Thay đổi
```

## Tiêu chí hoàn thành
1. Plan có `## Tạo sản phẩm` → Dev nhận lệnh bắt buộc chạy; prompt Dev không còn cấm.
2. Thiếu bất kỳ file `## Sản phẩm` nào thì task KHÔNG thể `qa_pass`.
3. QA pass → sản phẩm được gửi qua Telegram (video ≤ 50 MB bằng `sendVideo`, lớn hơn thì gửi đường dẫn).
4. Prompt Lead hướng dẫn plan đi tới sản phẩm cuối và coi duyệt plan là duyệt mọi cổng nội bộ.
5. Test pass, fmt/clippy sạch, chỉ sửa file trong bảng.

## Commit
Mỗi mục A, B+E(dev), C+E(qa), D, E(lead) một commit (`feat(agent-brain): …`), chỉ add đúng file. Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh kiểm tra, phần chưa kiểm (chưa gửi Telegram thật, chưa chạy với dự án `srt` thật).
