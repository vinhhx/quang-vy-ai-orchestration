# Requirements: Rust Agent Brain

**Defined:** 2026-10-14
**Core Value:** Giao việc cho nhiều AI agent bằng giao thức Lead → `.plan.md` → Dev → QA một cách đáng tin cậy: đúng dự án, đúng vai trò, kill sạch cây tiến trình, đo được token.

## Cách đọc tài liệu này

Không có PRD trong bộ tài liệu nạp, nên các yêu cầu được suy ra từ: (a) 42 test case nghiệm thu P1-01…P5-07 trong các spec nguồn (giữ nguyên ID test ở cuối mỗi dòng dưới dạng `(src: Pn-xx)`), và (b) mục tiêu, phạm vi, danh sách công việc, cổng phase và mitigation rủi ro của từng tài liệu nguồn (ghi `(src: ...)` kèm mục, đánh dấu **derived** vì không có test ID riêng).

**Lưu ý đánh số lệch.** Phase nguồn đánh số từ 0, GSD đánh số từ 1 (nguồn Phase N = GSD Phase N+1). ID test case giữ **số phase nguồn**: ví dụ `P2-02` (kill sạch) thuộc **GSD Phase 3**. Bảng ánh xạ đầy đủ ở cuối tài liệu và trong `ROADMAP.md`.

## v1 Requirements

Yêu cầu cho bản phát hành đầu tiên. Mỗi yêu cầu ánh xạ đúng một phase.

### Verification Spikes (GSD Phase 1 — nguồn Phase 0)

Phase này người dùng tự chạy tay để hiểu hành vi CLI; agent có thể giải thích output nhưng không được tự kết luận thay người dùng. Kết quả ghi vào `spike-results.md` theo mẫu (Ngày thử, Lệnh, Kết quả Đạt/Không đạt/Một phần, Quan sát, Ảnh hưởng tới thiết kế).

- [x] **SPK-01**: S1 — `claude -p ... --output-format stream-json --verbose` được thử thật và ghi nhận: có dòng chứa `session_id`; dòng cuối `type: result` có usage/token; mỗi tool call là một dòng riêng. *(derived: 01-phase0 §3 S1)*
- [x] **SPK-02**: S2 — Resume và fork được thử thật: `--resume <id>` nhớ ngữ cảnh; `--fork-session` tạo `session_id` mới và session gốc không bị đổi. *(derived: 01-phase0 §3 S2)*
- [x] **SPK-03**: S3 — Prompt cache được đo: chạy cùng task 2 lần cách nhau 1 phút, rồi 2 lần cách nhau 10 phút, so sánh cache read token trong dòng `result`; ghi nhận ảnh hưởng tới quota của gói subscription (nếu quan sát được). *(derived: 01-phase0 §3 S3)*
- [x] **SPK-04**: S4 — Kill cây tiến trình được thử thật: cho agent chạy `sleep 300` qua bash, kill tiến trình `claude`, chạy `ps -ef | grep sleep` và ghi lại tiến trình sót. *(derived: 01-phase0 §3 S4)*
- [x] **SPK-05**: S5 — Tách profile được thử thật: `CLAUDE_CONFIG_DIR=~/.claude-work` và `~/.claude-max` dùng đúng tài khoản và lịch sử session riêng. *(derived: 01-phase0 §3 S5)*
- [x] **SPK-06**: S6 — DeepSeek qua Claude Code được thử thật (`ANTHROPIC_BASE_URL` + key DeepSeek, task nhỏ có sửa file): tool call hoạt động và stream-json cùng định dạng. *(derived: 01-phase0 §3 S6)*
- [x] **SPK-07**: S7 — Antigravity được kiểm tra qua tài liệu chính thức: có lệnh không tương tác (headless), có xuất JSON, có cờ giới hạn turn hay không; nếu không có thì ghi rõ để Phase 6 thiết kế phương án khác. *(derived: 01-phase0 §3 S7)*
- [x] **SPK-08**: S8 — `claude -p "review git diff" --max-turns 1 --output-format json` được thử: dừng đúng 1 turn và kết quả dùng được. *(derived: 01-phase0 §3 S8)*
- [x] **SPK-09**: Cả 8 spike có kết quả Đạt / Không đạt / Một phần theo mẫu trong `spike-results.md`, và mỗi kết quả "Không đạt" có ghi phương án thay thế. *(derived: 01-phase0 §4)*
- [x] **SPK-10**: Đã quyết định profile Work/Personal dùng tài khoản Claude nào, sau khi xác nhận chính sách công ty (hai tài khoản đều do công ty cấp — có được dùng cho dự án cá nhân và dựng video không). *(derived: 01-phase0 §4; 00-ROADMAP §7 câu hỏi mở)*

### Project Routing (GSD Phase 2 — nguồn Phase 1)

- [x] **ROUTE-01**: Người dùng gọi dự án bằng tên tiếng Việt có dấu (ví dụ "web bán hàng") và hệ thống khớp đúng dự án `web-ban-hang` (bỏ dấu cả hai phía, so với `name_vi` và `aliases` trong `projects_map.json`). *(src: P1-01)*
- [x] **ROUTE-02**: Người dùng gọi dự án bằng alias (ví dụ "shop") và hệ thống khớp đúng dự án. *(src: P1-02)*
- [x] **ROUTE-03**: Khi tên khớp hơn 1 dự án, hệ thống dừng, liệt kê các dự án khớp và hỏi lại; không chạy agent (luật Fallback Ask — không bao giờ đoán). *(src: P1-03)*
- [x] **ROUTE-04**: Khi tên không khớp dự án nào, hệ thống dừng, liệt kê toàn bộ dự án và hỏi lại; không chạy agent. *(src: P1-04)*
- [x] **ROUTE-05**: Sau khi clone dự án mới và chạy quét thư mục gốc, dự án mới xuất hiện trong `projects_map.json` và các alias/`name_vi` đã sửa tay không bị ghi đè. *(src: P1-05)*
- [x] **ROUTE-06**: Script quét cảnh báo khi có alias trùng giữa các dự án. *(derived: 02-phase1 §3.2, §8)*

### Role Protocol (GSD Phase 2 — nguồn Phase 1)

- [x] **ROLE-01**: Sau khi Lead chạy xong, không file code nào bị sửa (`git status` sạch ngoài `.plan.md`); `.plan.md` theo đúng mẫu (Mục tiêu / Thay đổi gồm file, dòng/hàm, sửa gì, lý do / Không được đụng tới / Cách kiểm tra / Vướng mắc). *(src: P1-06)*
- [x] **ROLE-02**: Dev chạy với plan đầy đủ chỉ sửa các file được nêu trong plan và commit trên nhánh task với message "<task>: <mô tả ngắn>". *(src: P1-07)*
- [x] **ROLE-03**: Dev chạy với plan thiếu thông tin ghi vào mục "Vướng mắc" của `.plan.md` rồi dừng, không sửa bừa và không sửa ngoài plan. *(src: P1-08)*
- [x] **ROLE-04**: Sau khi QA chạy, có `.qa-report.md` (khớp plan từng mục, sửa ngoài plan, rủi ro, kết luận Đạt / Cần sửa) và không có code bị sửa. *(src: P1-09)*
- [x] **ROLE-05**: Người dùng chạy được `brain lead "<dự án>" "<yêu cầu>"` (định tuyến → tạo nhánh task → chạy Lead), `brain dev "<dự án>"` (Dev đọc `.plan.md`) và `brain qa "<dự án>"` (QA trên `git diff`); 3 prompt vai trò lưu thành file riêng. *(derived: 02-phase1 §4.3, §4.4, §5)*
- [x] **ROLE-06**: Lệnh cấm của từng vai trò được thực thi bằng giới hạn công cụ (`--allowedTools` / `--disallowedTools`), không chỉ bằng prompt; các vai trò chỉ trao đổi thông tin qua file (`.plan.md`, `git diff`, `.qa-report.md`). *(derived: 02-phase1 §3.3, §8)*
- [x] **ROLE-07**: Hoàn thành 5 task thật bằng giao thức Lead → Dev → QA, mỗi task có nhật ký (số turn của Dev trước khi sửa, Dev có đọc file ngoài plan không, QA phát hiện gì, có phải làm lại không và vì sao). *(derived: 00-ROADMAP §2 cổng Phase 1; 02-phase1 §5, §7)*

### Profiles (GSD Phase 2 và 3)

- [x] **PROF-01**: Dự án có `profile: work` chạy với đúng `CLAUDE_CONFIG_DIR` của profile work (mức biến môi trường, trong script). *(src: P1-10)*
- [x] **PROF-02**: `agent-brain` chọn `CLAUDE_CONFIG_DIR` / biến môi trường engine theo trường `profile` của dự án (`profile.rs`). *(derived: 03-phase2 §4.3, §5)*

### Rust CLI Core (GSD Phase 3 — nguồn Phase 2)

- [x] **CLI-01**: `agent-brain run lead "<dự án>" "<yêu cầu>"` chạy trên dự án thật và log hiện từng bước (khởi tạo, tin nhắn assistant, tool call, kết quả tool, kết quả cuối) trên terminal. *(src: P2-01)*
- [x] **CLI-02**: `agent-brain projects scan` cùng việc so khớp tên không dấu và Fallback Ask cho kết quả giống hành vi script Phase 2 (ROUTE-01…06 vẫn đúng khi chạy qua binary, module `projects.rs`). *(derived: 03-phase2 §4.1, §5)*
- [x] **CLI-03**: `agent-brain run <lead|dev|qa> "<dự án>" "<yêu cầu>"` chạy đủ 3 vai trò bằng 3 prompt chuyển từ Phase 2 vào binary, giữ nguyên ràng buộc giao thức (ROLE-01…04, ROLE-06). *(derived: 03-phase2 §4.1, §5 "Chuyển 3 prompt")*

### Process Management (GSD Phase 3)

- [x] **PROC-01**: `agent-brain kill <task_id>` khi agent đang chạy `sleep 300`: sau 5 giây, `ps` không còn tiến trình `claude` hay `sleep` liên quan (tiến trình con chạy trong process group riêng; SIGTERM cả group → chờ vài giây → SIGKILL cả group; `kill_on_drop` là lớp thứ hai). *(src: P2-02)*
- [x] **PROC-02**: Đóng terminal hoặc Ctrl+C giữa chừng một run thì không còn tiến trình sót. *(src: P2-03)*

### Log Capture & Storage (GSD Phase 3)

- [x] **LOG-01**: Khi một run kết thúc, bảng `runs` có đủ input/output/cache read/cache write token, `cost_usd`, `session_id`, `pid`, trạng thái và thời gian bắt đầu/kết thúc; các dòng log lưu vào `events` kèm `raw_json`. *(src: P2-04)*
- [x] **LOG-02**: Dòng JSON lỗi trong stream được lưu dạng thô và chương trình không dừng; trường lạ trong JSON được bỏ qua (parse mềm). *(src: P2-05)*

### Fast-path (GSD Phase 3)

- [x] **FAST-01**: `agent-brain status "<dự án>"` trả kết quả `git status`/`git log`/`git diff --stat` do Rust tự chạy, không gọi AI, 0 token (cấp 1). *(src: P2-06)*
- [x] **FAST-02**: `agent-brain inspect "<dự án>"` gom `git diff` vào prompt và chạy đúng 1 turn (`--max-turns 1`), không tạo session lưu lại (cấp 2). *(src: P2-07)*

### Stats (GSD Phase 3)

- [x] **STAT-01**: `agent-brain stats --profile work` chỉ tính các run của profile work (và tương tự cho `personal`). *(src: P2-08)*

### Distribution (GSD Phase 3)

- [x] **DIST-01**: Copy binary `agent-brain` sang máy/thư mục khác vẫn chạy được, không cần cài thêm gì (trừ CLI của agent). *(src: P2-09)*

### Session & Token Optimization (GSD Phase 4 — nguồn Phase 3)

Điều kiện bắt đầu: có ít nhất 2 tuần dữ liệu `runs` thật từ GSD Phase 3.

- [ ] **OPT-01**: Báo cáo token trung bình theo vai trò (Lead/Dev/QA) và theo dự án từ SQLite, kèm số token của 3 turn đầu mỗi run (chi phí "mò đường"), có số liệu trước/sau tối ưu. *(src: P3-01)*
- [ ] **OPT-02**: Với `.gsd-summary.md` (phương án A), Dev bắt đầu sửa code trong ≤ 2 turn. *(src: P3-02)*
- [ ] **OPT-03**: Sửa `.gsd` thì `.gsd-summary.md` tự được tạo lại ở lần chạy sau (theo hash `.gsd`). *(src: P3-03)*
- [ ] **OPT-04**: Khi session vượt ngưỡng token (ban đầu 45.000, chỉnh theo số liệu), hệ thống tạo `.handover.md` (Đã hoàn thành / Đang dở / Quyết định đã chốt / File liên quan / Bước tiếp theo), mở session mới nạp handover và tiếp tục đúng việc; session cũ được giữ lại; bảng `sessions` ghi `context_tokens`. *(src: P3-04)*
- [ ] **OPT-05**: Session mới khởi tạo từ handover không hỏi lại những quyết định đã chốt. *(src: P3-05)*
- [ ] **OPT-06**: Token trung bình mỗi task giảm đo được so với số liệu Phase 3 làm mốc; không được đóng phase khi chưa có kết quả này (cổng phase). *(src: P3-06)*
- [ ] **OPT-07**: Quyết định có làm phương án B (golden session + fork) hay không, dựa trên số liệu đo lại, và ghi lại lý do. *(derived: 04-phase3 §4)*

### Embedded Dashboard (GSD Phase 5 — nguồn Phase 4)

- [x] **DASH-01**: Chạy binary thì mở được dashboard, không cần cài thêm gì (UI HTML + Tailwind + Alpine.js nhúng trong binary, cùng origin với API). *(src: P4-01)*
- [x] **DASH-02**: Truy cập dashboard từ máy khác trong mạng LAN không được (server chỉ nghe `127.0.0.1`). *(src: P4-02)*
- [x] **DASH-03**: Truy cập không có token bị từ chối (token sinh khi khởi động và in ra terminal). *(src: P4-03)*
- [x] **DASH-04**: Giao việc từ UI (ô nhập yêu cầu tiếng Việt) thì log hiện realtime qua SSE và từng bước pipeline Lead → Dev → QA đổi trạng thái. *(src: P4-04)*
- [x] **DASH-05**: Reload trang giữa chừng một run thì vẫn thấy log cũ (đọc từ SQLite `events`) và tiếp tục nhận log mới; SSE tự kết nối lại và đọc bù từ SQLite. *(src: P4-05)*
- [x] **DASH-06**: Bấm Kill trên UI thì run dừng và không còn tiến trình sót (như PROC-01). *(src: P4-06)*
- [x] **DASH-07**: Tên dự án mơ hồ trên UI thì hiện danh sách để chọn, không tự chạy. *(src: P4-07)*
- [x] **DASH-08**: Đổi profile trên UI thì thống kê và task mới theo đúng profile. *(src: P4-08)*
- [x] **DASH-09**: Tắt internet thì giao diện vẫn hiển thị đúng style (CSS Tailwind build sẵn và nhúng). *(src: P4-09)*
- [x] **DASH-10**: Nếu có thiết kế (Figma), UI khớp theo checklist UI của người dùng; nếu chưa có thiết kế thì không áp dụng. *(src: P4-10)*
- [x] **DASH-11**: Màn hình Thống kê hiển thị token theo ngày, theo profile, theo dự án và theo vai trò; màn hình Tổng quan hiển thị task đang chạy / xong / lỗi. *(derived: 05-phase4 §4.1)*
- [x] **DASH-12**: Màn hình Dự án liệt kê dự án từ `projects_map.json`, cho sửa alias và có nút quét lại. *(derived: 05-phase4 §4.1)*

### Pipeline View (GSD Phase 5.1 — Dashboard v2, bước A)

Thiết kế: https://claude.ai/artifact/KdMc5CMNSMg4G5T9jLjFRK (quyết định của người dùng 2026-10-19). Chỉ dùng dữ liệu `runs`/`tasks` đã có — không đổi giao thức, không migration CSDL.

- [ ] **PIPE-01**: Màn hình Chi tiết task thay stepper bằng pipeline thẻ giai đoạn (Bắt đầu → Lên plan → Bạn → Code → QA): mỗi thẻ có trạng thái (ký hiệu + chữ + màu), một dòng kết quả, thời gian và chi phí; lần chạy lại cùng vai trò hiện là vòng ("vòng N"). *(derived: Dashboard v2 View 1)*
- [ ] **PIPE-02**: Sổ lượt chạy dưới pipeline liệt kê mọi run của task theo thứ tự, nhóm theo chu kỳ (plan / build / QA), mỗi dòng có nhãn vòng + vai trò, từ kết quả (verdict), số lượt, phút, chi phí; dòng cuối "Tổng đến giờ" = số run claude · phút · $; bấm một dòng mở log của run đó. *(derived: Dashboard v2 View 1)*
- [ ] **PIPE-03**: Thẻ "Bạn" thể hiện cổng duyệt plan suy ra từ dữ liệu có sẵn: "chờ bạn" khi plan đã commit và chưa có Dev trên plan đó, "đã duyệt" kèm giờ khi Dev đã chạy trên commit plan hiện tại. *(derived: Dashboard v2 View 1)*
- [ ] **PIPE-04**: Bộ từ vựng trạng thái chung (xong, đang chạy, chờ bạn, đã dừng, chưa bắt đầu, chưa tự động) có chú thích; "đang chạy" dùng màu xanh dương (indigo chỉ còn cho hành động/lựa chọn); giai đoạn chưa có (review plan, review code, sửa, PR nháp) không giả lập mà hiện "chưa tự động". *(derived: Dashboard v2 design system)*
- [ ] **PIPE-05**: Pipeline và sổ lượt chạy cập nhật trực tiếp qua SSE như các màn hình khác; giữ nguyên nút Kill / Chạy Dev / Chạy QA / Chạy lại Lead và các quy tắc bận; hoạt động ở 1024/768/480 px (dưới 640 px xếp dọc), cả giao diện sáng/tối, dùng được bằng bàn phím; dữ liệu đến từ `GET /api/tasks/{id}/pipeline`. *(derived: Dashboard v2 View 1)*

### Multi-engine Routing (GSD Phase 6 — nguồn Phase 5)

- [ ] **ENG-01**: Chạy task với đội hình `tiet-kiem` (trong `teams.json`): Lead và QA chạy Claude, Dev chạy DeepSeek; log ghi đúng engine cho từng run. Đây là cổng phase: 1 task trộn engine chạy end-to-end. *(src: P5-01)*
- [ ] **ENG-02**: Giả lập hết quota giữa task thì hệ thống báo lỗi quota/rate-limit (nhận diện từ stream hoặc mã thoát), đề xuất đổi đội hình và không tự đổi ngầm. *(src: P5-02)*
- [ ] **ENG-03**: Đổi engine giữa task thì engine mới tiếp tục từ `.plan.md` (và `.handover.md` nếu có), không làm lại từ đầu. *(src: P5-03)*
- [ ] **ENG-04**: Thống kê theo engine tách đúng số token của từng engine. *(src: P5-04)*
- [ ] **ENG-05**: Key DeepSeek không xuất hiện trong file đã commit hay trong log (key đọc từ biến môi trường; lọc biến nhạy cảm trước khi ghi log). *(src: P5-07)*
- [ ] **ENG-06**: Antigravity được tích hợp theo kết quả S7 (SPK-07): nếu headless thì chạy như engine; nếu không thì dùng thủ công (người dùng mở và giao `.plan.md`) và theo dõi qua `git diff` thay vì log. *(derived: 06-phase5 §2.2, §4)*
- [ ] **ENG-07**: Tỉ lệ "Cần sửa" của QA được ghi theo engine để so sánh chất lượng; QA mặc định chạy bằng engine mạnh. *(derived: 06-phase5 §6)*

### Media Branch (GSD Phase 6)

- [ ] **MEDIA-01**: Dự án `type: media` không tạo Git worktree; file render nằm ngoài repo (thư mục riêng như `/video-studio`). *(src: P5-05)*
- [ ] **MEDIA-02**: Kill một job render thì FFmpeg dừng và không còn tiến trình sót (runner Phase 3 quản lý render như tiến trình riêng). *(src: P5-06)*
- [ ] **MEDIA-03**: Kịch bản whiteboard đã duyệt được đưa vào bước dựng: agent viết cấu hình render → render (FFmpeg/Remotion/Python) → file output; mỗi job ghi vào bảng `media_jobs`. *(derived: 06-phase5 §2.4, §3.2, §3.3)*

## v2 Requirements

Hoãn lại, chưa nằm trong roadmap hiện tại.

### Fast-path (cần quyết định)

- **FAST-03**: Fast-path cấp 3 — warm reuse: hỏi tiếp vào session vừa chạy bằng `--resume`. *Khoảng trống phạm vi:* `03-phase2-loi-rust.md` §3.4 hoãn cấp 3 sang "Phase 3" (nguồn) nhưng `04-phase3-toi-uu-session.md` chỉ bàn fork/golden session/handover, không có công việc hay test case nào cho `--resume` warm reuse. Chưa gán vào phase nào; quyết định ở `/gsd-discuss-phase 4` (thêm vào GSD Phase 4 hay giữ ở v2).

### Session Optimization

- **SESS-01**: Golden session + fork (phương án B/C của Phase 3 nguồn), chỉ làm nếu OPT-07 kết luận số liệu cho thấy còn lãng phí token đáng kể và các task thường chạy sát nhau (trong ~5 phút cache).

## Out of Scope

Loại trừ rõ ràng, kèm lý do để tránh thêm lại.

| Feature | Reason |
|---------|--------|
| Đăng nhập nhiều người dùng | Ngoài phạm vi Phase 4 nguồn; dự án dành cho một người dùng |
| Truy cập dashboard từ xa qua internet | Dashboard chạy được lệnh trên máy; chỉ `127.0.0.1` + token (DASH-02, DASH-03) |
| Tự động chuyển engine ngầm khi hết quota | Chất lượng các engine khác nhau; phải hỏi xác nhận hoặc theo luật người dùng cấu hình (ENG-02) |
| File video hàng GB trong Git worktree | Làm nặng Orca/Git; nhánh Media chạy ở thư mục riêng (MEDIA-01) |
| Chạy song song nhiều task | Nguồn chỉ loại khỏi phạm vi Phase 1 nguồn, chưa có phase nào lên lịch; có thể thêm sau |
| Tailwind CDN cho bản dùng thật | Cần internet; build CSS sẵn và nhúng (DASH-09) |
| Di chuyển dữ liệu/quy trình từ Paperclip | Nguồn nói "thay thế Paperclip" nhưng không có yêu cầu di chuyển; thêm bằng `/gsd-phase` nếu cần |

## Traceability

Mỗi yêu cầu v1 ánh xạ đúng một phase. Bảng này giữ đúng 3 cột; ID test nguồn nằm ở cuối mỗi dòng yêu cầu và ở bảng tra cứu phía dưới.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SPK-01 | Phase 1 | Complete |
| SPK-02 | Phase 1 | Complete |
| SPK-03 | Phase 1 | Complete |
| SPK-04 | Phase 1 | Complete |
| SPK-05 | Phase 1 | Complete |
| SPK-06 | Phase 1 | Complete |
| SPK-07 | Phase 1 | Complete |
| SPK-08 | Phase 1 | Complete |
| SPK-09 | Phase 1 | Complete |
| SPK-10 | Phase 1 | Complete |
| ROUTE-01 | Phase 2 | Complete |
| ROUTE-02 | Phase 2 | Complete |
| ROUTE-03 | Phase 2 | Complete |
| ROUTE-04 | Phase 2 | Complete |
| ROUTE-05 | Phase 2 | Complete |
| ROUTE-06 | Phase 2 | Complete |
| ROLE-01 | Phase 2 | Complete |
| ROLE-02 | Phase 2 | Complete |
| ROLE-03 | Phase 2 | Complete |
| ROLE-04 | Phase 2 | Complete |
| ROLE-05 | Phase 2 | Complete |
| ROLE-06 | Phase 2 | Complete |
| ROLE-07 | Phase 2 | Complete |
| PROF-01 | Phase 2 | Complete |
| CLI-01 | Phase 3 | Complete |
| CLI-02 | Phase 3 | Complete |
| CLI-03 | Phase 3 | Complete |
| PROF-02 | Phase 3 | Complete |
| PROC-01 | Phase 3 | Complete |
| PROC-02 | Phase 3 | Complete |
| LOG-01 | Phase 3 | Complete |
| LOG-02 | Phase 3 | Complete |
| FAST-01 | Phase 3 | Complete |
| FAST-02 | Phase 3 | Complete |
| STAT-01 | Phase 3 | Complete |
| DIST-01 | Phase 3 | Complete |
| OPT-01 | Phase 4 | Pending |
| OPT-02 | Phase 4 | Pending |
| OPT-03 | Phase 4 | Pending |
| OPT-04 | Phase 4 | Pending |
| OPT-05 | Phase 4 | Pending |
| OPT-06 | Phase 4 | Pending |
| OPT-07 | Phase 4 | Pending |
| DASH-01 | Phase 5 | Complete |
| DASH-02 | Phase 5 | Complete |
| DASH-03 | Phase 5 | Complete |
| DASH-04 | Phase 5 | Complete |
| DASH-05 | Phase 5 | Complete |
| DASH-06 | Phase 5 | Complete |
| DASH-07 | Phase 5 | Complete |
| DASH-08 | Phase 5 | Complete |
| DASH-09 | Phase 5 | Complete |
| DASH-10 | Phase 5 | Complete |
| DASH-11 | Phase 5 | Complete |
| DASH-12 | Phase 5 | Complete |
| PIPE-01 | Phase 5.1 | Pending |
| PIPE-02 | Phase 5.1 | Pending |
| PIPE-03 | Phase 5.1 | Pending |
| PIPE-04 | Phase 5.1 | Pending |
| PIPE-05 | Phase 5.1 | Pending |
| ENG-01 | Phase 6 | Pending |
| ENG-02 | Phase 6 | Pending |
| ENG-03 | Phase 6 | Pending |
| ENG-04 | Phase 6 | Pending |
| ENG-05 | Phase 6 | Pending |
| ENG-06 | Phase 6 | Pending |
| ENG-07 | Phase 6 | Pending |
| MEDIA-01 | Phase 6 | Pending |
| MEDIA-02 | Phase 6 | Pending |
| MEDIA-03 | Phase 6 | Pending |

**Coverage:**
- v1 requirements: 65 total (42 từ test case nguồn + 23 derived)
- Mapped to phases: 65
- Unmapped: 0 ✓
- Ngoài v1 (chưa gán, cần quyết định): FAST-03 (xem mục v2)

### Tra cứu test case nguồn → yêu cầu (42/42)

Ánh xạ 1-1 giữa ID test nguồn và yêu cầu (số phase trong ID là số phase **nguồn**):

| Test ID nguồn | Yêu cầu | GSD Phase |
|---------------|---------|-----------|
| P1-01 … P1-05 | ROUTE-01 … ROUTE-05 | 2 |
| P1-06 … P1-09 | ROLE-01 … ROLE-04 | 2 |
| P1-10 | PROF-01 | 2 |
| P2-01 | CLI-01 | 3 |
| P2-02, P2-03 | PROC-01, PROC-02 | 3 |
| P2-04, P2-05 | LOG-01, LOG-02 | 3 |
| P2-06, P2-07 | FAST-01, FAST-02 | 3 |
| P2-08 | STAT-01 | 3 |
| P2-09 | DIST-01 | 3 |
| P3-01 … P3-06 | OPT-01 … OPT-06 | 4 |
| P4-01 … P4-10 | DASH-01 … DASH-10 | 5 |
| P5-01 … P5-04 | ENG-01 … ENG-04 | 6 |
| P5-05, P5-06 | MEDIA-01, MEDIA-02 | 6 |
| P5-07 | ENG-05 | 6 |

---
*Requirements defined: 2026-10-14*
*Last updated: 2026-10-19 — added PIPE-01..05 (Phase 5.1)*
