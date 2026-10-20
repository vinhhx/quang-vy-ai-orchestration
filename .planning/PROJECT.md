# Rust Agent Brain

> EN summary: a lightweight, single-binary orchestrator (`agent-brain`) that dispatches work to multiple AI agents (Claude, DeepSeek, Antigravity...) across projects (code, video, customers). It replaces Paperclip. CLI first, embedded web dashboard later; drives Claude Code CLI and other agent CLIs.

## What This Is

Rust Agent Brain (binary `agent-brain`) là một bộ điều phối (orchestrator) nhẹ, chạy bằng **một file binary duy nhất**, dùng để giao việc cho nhiều AI agent (Claude, DeepSeek, Antigravity…) trên mọi dự án của một lập trình viên làm việc một mình: code, video, khách hàng. Nó thay thế Paperclip. Giai đoạn đầu là CLI lái Claude Code CLI (và các agent CLI khác); sau đó có dashboard web nhúng ngay trong binary.

Người dùng là một người duy nhất (visionary + tester nghiệm thu); Claude là bên viết code. Người dùng không đọc được Rust, nên mọi module phải có test theo hành vi và nghiệm thu bằng bảng test case.

## Core Value

Giao việc cho nhiều AI agent bằng giao thức **Lead → `.plan.md` → Dev → QA** một cách đáng tin cậy: đúng dự án, đúng vai trò, kill sạch cây tiến trình, và đo được token — giao diện chỉ là lớp hiển thị phía trên.

## Success Metrics

KPI xuyên suốt (nguồn: `files/00-ROADMAP.md` mục 5):

| Chỉ số | Cách đo | Mục tiêu |
|--------|---------|----------|
| Token trung bình / task | SQLite (từ GSD Phase 3 trở đi) | Giảm dần qua các phase |
| Số turn trước khi agent bắt đầu sửa code | Đếm trong log stream-json | ≤ 2 |
| Task chạy nhầm repo | Ghi tay / log | 0 |
| Tiến trình còn sót sau Kill | `ps` sau khi kill | 0 |
| Task phải làm lại do Dev hiểu sai plan | Ghi tay (nhật ký task) | Giảm dần |

Cổng qua phase (phase gates) nằm trong `ROADMAP.md` (mỗi phase có Success Criteria; cổng chính: Phase 1 = mọi giả định có kết quả Đạt/Không đạt/Một phần; Phase 2 = 5 task thật qua giao thức; Phase 3 = kill sạch + log token đầy đủ; Phase 4 = giảm token/task đo được so với Phase 3; Phase 5 = xem log realtime + kill từ UI; Phase 6 = 1 task trộn engine chạy end-to-end).

## Requirements

### Validated

- ✓ **Phase 1 — Kiểm chứng giả định** (SPK-01…10): 8 spike S1–S8 đã chạy thật, kết quả trong `spikes/spike-results.md` (S1–S4 Đạt, S5–S8 Một phần + follow-up); SPK-10 quyết định Work = `~/.claude`, Personal = `~/.claude-orca` — Phase 1
- ✓ **Phase 5 — Dashboard nhúng** (DASH-01…12): `agent-brain serve` — Axum + SSE + Tailwind/Alpine CSP nhúng trong binary tĩnh, chỉ `127.0.0.1` + token mỗi lần khởi động, 5 màn hình (Tổng quan, Giao việc, Task, Thống kê, Dự án); nghiệm thu P4-01…P4-09 Đạt, P4-10 không áp dụng (không có Figma) — `protocol-log/nghiem-thu-p4.md` — Phase 5
- ✓ **Phase 5.1 — Pipeline View** (PIPE-01…05): màn Chi tiết task kiểu "Autopilot" — thẻ giai đoạn có vòng, thời gian, chi phí, cổng "Bạn" suy ra từ git, sổ lượt chạy theo chu kỳ kèm tổng và phân trang; dựng từ dữ liệu runs có sẵn, không đổi schema/giao thức (bước A của Dashboard v2) — Phase 5.1

### Active

65 yêu cầu v1 trong `.planning/REQUIREMENTS.md`, suy ra từ 42 test case nghiệm thu của các tài liệu nguồn (P1-01…P5-07) cộng với 23 yêu cầu suy ra từ mục tiêu, danh sách công việc và cổng phase. Nhóm theo phase:

- [ ] **Phase 2 — Giao thức MVP bằng script** (ROUTE, ROLE, PROF-01): định tuyến dự án tiếng Việt + Fallback Ask, 3 vai trò Lead/Dev/QA, profile, 5 task thật
- [ ] **Phase 3 — Lõi Rust CLI** (CLI, PROF-02, PROC, LOG, FAST, STAT, DIST): process group + kill sạch, bắt log stream-json, SQLite token, fast-path cấp 1–2, binary portable
- [ ] **Phase 4 — Tối ưu session & token** (OPT): báo cáo token, `.gsd-summary.md`, `.handover.md`, giảm token/task đo được
- [ ] **Phase 6 — Đa đội hình & nhánh Media** (ENG, MEDIA): routing engine theo `teams.json`, xử lý quota, nhánh video tách khỏi Git worktree

### Out of Scope

- Đăng nhập nhiều người dùng và truy cập từ xa qua internet — dashboard có thể chạy lệnh trên máy, chỉ cho phép `127.0.0.1` + token (nguồn: Phase 4 spec)
  - Ngoại lệ hẹp (quyết định người dùng 2026-10-20, Phase 5.3): bot Telegram long polling (không mở cổng vào), chỉ chat ID của người dùng, chỉ thông báo + hành động ở cổng duyệt/khi autopilot dừng (Duyệt plan, Trả về Lead, Chạy thêm 1 vòng, Dừng hẳn, Kill có xác nhận); không giao việc mới, không truy cập dashboard từ xa.
- Tự động chuyển engine ngầm khi hết quota — chất lượng các engine khác nhau, phải hỏi xác nhận hoặc theo luật người dùng cấu hình (nguồn: Phase 5 spec)
- Đưa file video nặng hàng GB vào Git worktree — làm nặng Orca/Git (nguồn: Phase 5 spec)
- Chạy song song nhiều task — nguồn chỉ loại khỏi phạm vi Phase giao thức MVP, chưa có phase nào lên lịch; xem ghi chú trong `REQUIREMENTS.md`
- Tailwind qua CDN cho bản dùng thật — cần internet; bản dùng thật phải build CSS sẵn và nhúng vào binary (nguồn: Phase 4 spec)

## Context

**Cấu trúc phase nguồn và ánh xạ sang GSD.** Tài liệu nguồn đánh số phase từ 0; GSD đánh số từ 1. Mỗi số phase nguồn lệch +1 khi sang GSD, còn ID test case (P1-xx…P5-xx) **giữ nguyên số phase nguồn**:

| GSD Phase | Tài liệu nguồn | Phase nguồn | ID test case nguồn |
|-----------|----------------|-------------|--------------------|
| 1 — Kiểm chứng giả định | `01-phase0-kiem-chung.md` | Phase 0 | (không có; dùng 8 spike S1–S8 + 3 mục nghiệm thu) |
| 2 — Giao thức MVP (script) | `02-phase1-giao-thuc-mvp.md` | Phase 1 | P1-01…P1-10 |
| 3 — Lõi Rust CLI | `03-phase2-loi-rust.md` | Phase 2 | P2-01…P2-09 |
| 4 — Tối ưu session & token | `04-phase3-toi-uu-session.md` | Phase 3 | P3-01…P3-06 |
| 5 — Dashboard nhúng | `05-phase4-dashboard.md` | Phase 4 | P4-01…P4-10 |
| 6 — Đa đội hình & Media | `06-phase5-da-doi-hinh-media.md` | Phase 5 | P5-01…P5-07 |

**Thứ tự phụ thuộc.** Phase 1 → 2 → 3, rồi Phase 4, 5, 6 độc lập nhau sau Phase 3. Chọn thứ tự theo nỗi đau lớn nhất lúc đó: hết quota thường xuyên → Phase 6 trước; token/task cao → Phase 4 trước; khó theo dõi agent → Phase 5 trước. Quyết định sau Phase 3.

**Hai ràng buộc ghi nhận (không chặn).**
- Phase 6 phụ thuộc mềm vào `.handover.md` của Phase 4 (engine mới đọc `.plan.md` / `.handover.md` để tiếp tục); P5-03 chỉ yêu cầu `.plan.md` nên Phase 6 làm được mà không cần Phase 4.
- Fast-path cấp 3 (warm reuse, `--resume`) bị hoãn từ Phase 2 nguồn sang Phase 3 nguồn (= GSD Phase 4) nhưng spec Phase 3 nguồn không có công việc rõ ràng cho nó. Xem `REQUIREMENTS.md` (mục v2, FAST-03) và `STATE.md`.

**Môi trường kỹ thuật.** Greenfield (thư mục dự án mới, chưa có mã Rust). Phụ thuộc vào Claude Code CLI (`claude -p`, `--output-format stream-json --verbose`, `--resume`, `--fork-session`, `--max-turns`, `--allowedTools`, `CLAUDE_CONFIG_DIR`, `ANTHROPIC_BASE_URL`). Hệ điều hành đích: Unix-like (suy ra từ `ps`, SIGTERM/SIGKILL, process group — nguồn không nói rõ).

**Tham chiếu nằm ngoài bộ tài liệu nạp vào** (không có trong repo, cần hỏi lại khi cần): "bảng kiến trúc gốc" với các vấn đề đánh số #1–#11 (dùng làm cột "Giải quyết" trong roadmap nguồn); định dạng `.gsd`; pipeline kịch bản whiteboard; Orca (công cụ quản lý Git worktree); Paperclip (công cụ bị thay thế — nguồn không có yêu cầu di chuyển dữ liệu/quy trình từ Paperclip). Tầm nhìn nhắc tới dự án "khách hàng" nhưng nguồn chỉ định nghĩa `type: code | media`.

**Ước lượng tham khảo của nguồn** (không phải cam kết; giả định làm ngoài giờ, agent viết code): Phase nguồn 0: 2–4 buổi; 1: 1–2 tuần; 2: 2–4 tuần (rủi ro nhất vì Rust async); 3: 1–2 tuần (phụ thuộc số liệu); 4: 2–3 tuần; 5: 2–4 tuần (phụ thuộc kết quả S7 về Antigravity).

**Nguồn nạp.** Intel tổng hợp ở `.planning/intel/` (SYNTHESIS.md là điểm vào), báo cáo xung đột ở `.planning/INGEST-CONFLICTS.md` (0 blocker; 1 warning về phân loại độ tin cậy thấp của tài liệu Phase 0 và Phase 5 đã được người dùng chấp thuận — giữ nguyên loại tài liệu), tài liệu gốc tiếng Việt ở `files/*.md`.

## Constraints

- **Tech stack**: Rust; Tokio, rusqlite (bundled), serde_json, clap (Phase 3); Axum + HTML + Tailwind + Alpine.js nhúng trong binary (Phase 5) — nguồn Phase 2/4 spec
- **Distribution**: Một file binary duy nhất, copy sang máy/thư mục khác vẫn chạy, không cần cài thêm gì (trừ CLI agent); UI cùng origin với API, không Node/venv/CORS — để đơn giản hóa vận hành
- **Offline**: Dashboard phải chạy hoàn toàn offline — CSS build sẵn và nhúng vào binary
- **Security**: Axum chỉ nghe `127.0.0.1`, token truy cập sinh khi khởi động và in ra terminal (bắt buộc) — dashboard có thể chạy lệnh trên máy người dùng
- **Process safety**: Tiến trình con chạy trong process group riêng; kill = SIGTERM cả group → chờ vài giây → SIGKILL cả group; giữ `kill_on_drop` làm lớp bảo vệ thứ hai — vì `kill_on_drop` chỉ kill tiến trình con trực tiếp, MCP server và shell con có thể sót
- **Log format tolerance**: Parser stream-json "mềm" (bỏ qua trường lạ), luôn lưu `raw_json`, dòng lỗi không làm sập chương trình — định dạng đổi theo phiên bản CLI và khác nhau giữa các engine
- **Measure before optimize**: Không tối ưu token khi chưa có số liệu `runs` thật; Phase 4 cần ≥ 2 tuần dữ liệu từ Phase 3 và không được đóng khi chưa có P3-06
- **Protocol**: Các vai trò chỉ trao đổi qua file (`.plan.md`, `git diff`, `.qa-report.md`); lệnh cấm được thực thi bằng giới hạn công cụ (`--allowedTools`/`--disallowedTools`), không chỉ bằng prompt
- **Secrets**: Không lưu key (ví dụ DeepSeek) trong file có thể bị commit; đọc từ biến môi trường; lọc biến nhạy cảm trước khi ghi log
- **Verification**: Mỗi phase có tiêu chí nghiệm thu dạng test case; người dùng là tester, agent viết code

## Key Decisions

Không có ADR trong bộ tài liệu nạp, nên **không có quyết định nào bị khóa (locked)**. Các lựa chọn thiết kế dưới đây lấy từ SPEC/DOC (có ghi nguồn), chưa phải ADR, và có thể được xem xét lại.

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Giao thức trước, giao diện sau (nguồn 00-ROADMAP §1) | Giá trị nằm ở cách agent phối hợp; dashboard chỉ là lớp hiển thị | ✓ Good — Phase 5 dashboard chỉ gọi lại lệnh CLI có sẵn (`run`, `cmd_kill`, stats) |
| Đo trước khi tối ưu (00-ROADMAP §1; Phase 3 spec) | Tránh tối ưu theo ước đoán; cần số liệu token thật | — Pending |
| Kiểm chứng giả định sớm ở Phase 1 (00-ROADMAP §1) | Một giả định sai có thể hỏng cả phase phía sau | ✓ Good — Phase 1 phát hiện E-05 sai, cache bucket 1h (không phải ~5 phút), `--max-turns` không dùng được cho review diff, dsh không có stream-json |
| Fallback Ask: khớp 0 hoặc >1 dự án thì dừng và hỏi, không bao giờ đoán (Phase 1 spec §3.2) | Tránh chạy nhầm repo (KPI = 0) | — Pending |
| CLI trước UI (Phase 2 spec §3.1) | Phần khó nhất là quản lý tiến trình và đọc stream async; dễ test bằng terminal | — Pending |
| Process group + SIGTERM/SIGKILL, giữ `kill_on_drop` làm lớp hai (Phase 2 spec §3.2) | `kill_on_drop` chỉ giết tiến trình con trực tiếp | — Pending (S4: ghi PID + con cháu, SIGTERM PID, chờ 5 s, quét sót; SIGKILL bỏ sót con) |
| Làm phương án A (tóm tắt `.gsd`) trước, chỉ làm B (golden session + fork) nếu số liệu cho thấy đáng (Phase 3 spec §2.3) | Cache chỉ sống ~5 phút; fork không chắc tiết kiệm token | — Pending (S3: cache thực tế là bucket 1h, ấm ở 10 phút, nguội ở 65 phút; S2: `--fork-session` đọc cache của session cha) |
| Ngưỡng độ dài session ban đầu 45.000 token, chỉnh theo số liệu (Phase 3 spec §2.4) | Context phình to làm tăng token | — Pending |
| Nhúng UI vào binary, localhost + token (Phase 4 spec §3) | Không Node/CORS; bảo mật tối thiểu vì UI chạy được lệnh | ✓ Good — Phase 5: binary tĩnh phục vụ UI không cần cài gì, CSS chạy offline; cookie không tách theo port là rủi ro chấp nhận AR-5-01 (cứng hoá: token qua header) |
| Mỗi vai trò chạy được trên engine khác nhau vì chỉ trao đổi qua file (Phase 5 spec §2.1) | Đổi engine một vai trò không ảnh hưởng vai trò khác | — Pending |
| Không tự chuyển engine ngầm khi hết quota; QA luôn chạy engine mạnh (Phase 5 spec §2.3, §6) | Chất lượng các engine khác nhau | — Pending |
| Nhánh Media chạy ở thư mục riêng ngoài Git worktree, render là tiến trình riêng do runner quản lý (Phase 5 spec §2.4) | File video hàng GB làm nặng Orca/Git | — Pending |
| Work = `~/.claude` (Team seat), Personal = `~/.claude-orca` (Pro) — SPK-10, người dùng miễn xác nhận chính sách công ty (Phase 1) | Hai profile tách biệt theo tài khoản; rủi ro tuân thủ đã chấp nhận (AR-1-01) | ✓ Accepted — S5 live test trên profile Personal chưa chạy |
| Phân công: Claude phân tích/lập kế hoạch/review, Antigravity (`agy`) thực thi (CLAUDE.md, sau Phase 1) | Tách người lập kế hoạch và người làm; agy headless được (S7) | ✓ Good — Phase 5: 13 plan + 4 fix plan qua agy với `--dangerously-skip-permissions` (người dùng duyệt cho Phase 5); review của Claude bắt được 2 lỗi UI khi chạy thật |

## Evolution

Tài liệu này tiến hóa theo vòng đời dự án.

**Sau mỗi lần chuyển phase:**
1. Yêu cầu nào không còn đúng? → chuyển sang Out of Scope kèm lý do
2. Yêu cầu nào đã được xác nhận? → chuyển sang Validated kèm tham chiếu phase
3. Yêu cầu mới xuất hiện? → thêm vào Active
4. Quyết định cần ghi lại? → thêm vào Key Decisions
5. "What This Is" còn chính xác không? → cập nhật nếu lệch

**Sau mỗi milestone:**
1. Rà soát toàn bộ các mục
2. Kiểm tra lại Core Value
3. Rà soát Out of Scope — lý do còn đúng không?
4. Cập nhật Context theo hiện trạng

---
*Last updated: 2026-10-20 after Phase 5.1*
