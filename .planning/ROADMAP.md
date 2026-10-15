# Roadmap: Rust Agent Brain

## Overview

Rust Agent Brain đi từ "chưa chắc cái gì chạy được" đến một binary `agent-brain` duy nhất điều phối nhiều AI agent trên mọi dự án. Lộ trình giữ nguyên cấu trúc của tài liệu nguồn: kiểm chứng giả định (Phase 1) → chứng minh giao thức Lead → Dev → QA bằng script (Phase 2) → chuyển thành lõi Rust CLI có kill sạch và đo token (Phase 3) → rồi ba nhánh độc lập: tối ưu session/token (Phase 4), dashboard nhúng (Phase 5), đa đội hình engine + nhánh Media (Phase 6). Nguyên tắc: giao thức trước giao diện sau, mỗi phase dùng được ngay, đo trước khi tối ưu, kiểm chứng giả định sớm, người dùng nghiệm thu bằng test case.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

**Ánh xạ phase nguồn → GSD** (tài liệu nguồn đánh số từ 0; ID test case giữ số phase nguồn, ví dụ `P2-02` thuộc GSD Phase 3):

| GSD Phase | Phase nguồn | Tài liệu nguồn | ID test nguồn |
|-----------|-------------|----------------|---------------|
| 1 | Phase 0 — Kiểm chứng giả định | `01-phase0-kiem-chung.md` | (spike S1–S8) |
| 2 | Phase 1 — Giao thức MVP (script) | `02-phase1-giao-thuc-mvp.md` | P1-01…P1-10 |
| 3 | Phase 2 — Lõi Rust (CLI) | `03-phase2-loi-rust.md` | P2-01…P2-09 |
| 4 | Phase 3 — Tối ưu session & token | `04-phase3-toi-uu-session.md` | P3-01…P3-06 |
| 5 | Phase 4 — Dashboard nhúng | `05-phase4-dashboard.md` | P4-01…P4-10 |
| 6 | Phase 5 — Đa đội hình & Media | `06-phase5-da-doi-hinh-media.md` | P5-01…P5-07 |

- [ ] **Phase 1: Verify Assumptions** - Thử thật 8 spike (S1–S8) và ghi `spike-results.md` trước khi viết dòng Rust nào
- [ ] **Phase 2: Protocol MVP (Scripts)** - Chạy Lead → `.plan.md` → Dev → QA trên dự án thật chỉ bằng script và file cấu hình
- [ ] **Phase 3: Rust CLI Core** - Binary `agent-brain` với kill sạch cây tiến trình, bắt log stream-json, token trong SQLite, fast-path
- [ ] **Phase 4: Session & Token Optimization** - Giảm token/task dựa trên số liệu `runs` thật (`.gsd-summary.md`, `.handover.md`)
- [ ] **Phase 5: Embedded Dashboard** - Web UI + SSE trong cùng binary: xem log realtime, giao việc, kill, thống kê
- [ ] **Phase 6: Multi-engine & Media Branch** - Mỗi vai trò một engine, xử lý hết quota không đổi ngầm, nhánh video tách khỏi Git worktree

**Execution Order & Dependencies:**

```
Phase 1 ──► Phase 2 ──► Phase 3 ──┬──► Phase 4
                                  ├──► Phase 5
                                  └──► Phase 6
```

- Phase 1 → 2 → 3 chạy tuần tự. Sau Phase 3, Phase 4, 5, 6 độc lập nhau; thứ tự chọn sau Phase 3 theo nỗi đau lớn nhất: hết quota thường xuyên → Phase 6 trước; token/task cao → Phase 4 trước; khó theo dõi agent → Phase 5 trước. (GSD chạy theo thứ tự số, nhưng `/gsd-plan-phase N` có thể gọi cho bất kỳ phase nào đã đủ phụ thuộc; dùng `/gsd-phase` nếu muốn đánh số lại.)
- **Điều kiện dữ liệu của Phase 4:** cần ít nhất 2 tuần dữ liệu `runs` thật từ việc dùng binary Phase 3 trong công việc thực; Phase 3 xong chưa đủ để bắt đầu Phase 4.
- **Phụ thuộc mềm Phase 6 → Phase 4:** engine mới đọc `.plan.md` / `.handover.md` để tiếp tục; `.handover.md` được định nghĩa ở Phase 4. P5-03 (ENG-03) chỉ yêu cầu `.plan.md` nên Phase 6 làm được mà không cần Phase 4; nếu Phase 4 xong trước, ENG-03 nên dùng cả `.handover.md`.
- **Phụ thuộc Phase 6 vào kết quả Phase 1:** S1, S5 (Claude/profile), S6 (DeepSeek), S7 (Antigravity) quyết định thiết kế engine; S7 quyết định ENG-06 là tích hợp headless hay dùng thủ công.
- **Khoảng trống phạm vi ghi nhận:** fast-path cấp 3 (warm reuse, `--resume`) bị hoãn từ Phase 3 (GSD) sang Phase 4 (GSD) nhưng spec Phase 4 không có công việc rõ ràng cho nó; xem FAST-03 trong `REQUIREMENTS.md` (mục v2) và quyết định ở `/gsd-discuss-phase 4`.

## Phase Details

### Phase 1: Verify Assumptions

**Goal**: Mọi giả định kỹ thuật mà kiến trúc dựa vào (cờ CLI, khả năng headless, hành vi cache/kill/profile) đã được thử thật và ghi lại kết quả, để không phase nào phía sau thiết kế trên một giả định sai.
**Source**: Phase 0 — `01-phase0-kiem-chung.md`
**Depends on**: Nothing (first phase)
**Requirements**: SPK-01, SPK-02, SPK-03, SPK-04, SPK-05, SPK-06, SPK-07, SPK-08, SPK-09, SPK-10
**Success Criteria** (what must be TRUE):
  1. `spike-results.md` có đủ 8 spike S1–S8, mỗi spike có kết quả Đạt / Không đạt / Một phần cùng ngày thử, lệnh, quan sát và ảnh hưởng tới thiết kế.
  2. Mỗi kết quả "Không đạt" có ghi phương án thay thế; riêng S7 nói rõ Antigravity có chạy headless và xuất log JSON không, để Phase 6 biết phải tích hợp hay dùng thủ công.
  3. Các số liệu nền đã có: stream-json cần `--verbose` ở chế độ `-p`, `session_id` và usage nằm ở dòng nào (S1), cache nóng/nguội 1 phút so với 10 phút (S3), tiến trình sót sau khi kill (S4), `--max-turns 1` dừng đúng 1 turn (S8).
  4. Đã quyết định profile Work/Personal dùng tài khoản Claude nào, sau khi xác nhận chính sách công ty cho phép.

**Plans**: TBD
- [x] 01-01-PLAN.md

### Phase 2: Protocol MVP (Scripts)

**Goal**: Người dùng chạy được quy trình Lead → `.plan.md` → Dev → QA trên dự án thật chỉ bằng script và file cấu hình, chứng minh giao thức hiệu quả trước khi đầu tư vào app Rust.
**Source**: Phase 1 — `02-phase1-giao-thuc-mvp.md`
**Depends on**: Phase 1
**Requirements**: ROUTE-01, ROUTE-02, ROUTE-03, ROUTE-04, ROUTE-05, ROUTE-06, ROLE-01, ROLE-02, ROLE-03, ROLE-04, ROLE-05, ROLE-06, ROLE-07, PROF-01
**Success Criteria** (what must be TRUE):
  1. Người dùng gọi dự án bằng tên tiếng Việt có dấu ("web bán hàng") hoặc alias ("shop") và script chọn đúng `web-ban-hang`; tên mơ hồ hoặc không khớp thì dừng, liệt kê dự án và không chạy agent.
  2. Sau khi clone dự án mới và chạy quét, dự án mới xuất hiện trong `projects_map.json`, alias đã sửa tay không mất, và alias trùng giữa các dự án được cảnh báo.
  3. `brain lead` chỉ ghi `.plan.md` (`git status` sạch ngoài plan); `brain dev` chỉ sửa file trong plan và ghi "Vướng mắc" rồi dừng khi plan thiếu; `brain qa` ghi `.qa-report.md` mà không sửa code.
  4. Dự án có `profile: work` chạy với đúng `CLAUDE_CONFIG_DIR` của work.
  5. 5 task thật đã hoàn thành qua giao thức, mỗi task có nhật ký (số turn Dev trước khi sửa, Dev có đọc file ngoài plan không, QA phát hiện gì, có làm lại không).

**Plans**: TBD

### Phase 3: Rust CLI Core

**Goal**: Giao thức của Phase 2 chạy được qua binary Rust `agent-brain` (CLI, chưa có UI) với quản lý tiến trình an toàn, bắt log stream-json, lưu token vào SQLite và fast-path không tốn token.
**Source**: Phase 2 — `03-phase2-loi-rust.md`
**Depends on**: Phase 2 (và kết quả S1, S4, S5, S8 + quyết định tài khoản từ Phase 1)
**Requirements**: CLI-01, CLI-02, CLI-03, PROF-02, PROC-01, PROC-02, LOG-01, LOG-02, FAST-01, FAST-02, STAT-01, DIST-01
**Success Criteria** (what must be TRUE):
  1. `agent-brain run lead|dev|qa "<dự án>" "<yêu cầu>"` chạy vai trò trên dự án thật, tên dự án tiếng Việt được định tuyến đúng (Fallback Ask khi mơ hồ), profile chọn theo dự án và log hiện từng bước trên terminal.
  2. Kill (`agent-brain kill <task_id>`) khi agent đang chạy `sleep 300`: sau 5 giây `ps` không còn `claude` hay `sleep` liên quan; Ctrl+C giữa chừng cũng không để sót tiến trình.
  3. Sau mỗi run, bảng `runs` có đủ token (input/output/cache), `session_id`, `pid`, thời gian; một dòng JSON lỗi trong stream được lưu thô và chương trình không dừng.
  4. `agent-brain status` trả kết quả git với 0 token và không gọi AI; `agent-brain inspect` chạy đúng 1 turn và không để lại session.
  5. `agent-brain stats --profile work` chỉ tính run của profile work; copy binary sang máy/thư mục khác vẫn chạy mà không cần cài thêm gì (trừ CLI agent).

**Plans**: TBD

### Phase 4: Session & Token Optimization

**Goal**: Token trung bình mỗi task giảm đo được so với mốc Phase 3, dựa trên số liệu thật trong SQLite chứ không phải ước đoán.
**Source**: Phase 3 — `04-phase3-toi-uu-session.md`
**Depends on**: Phase 3 (kèm điều kiện: ít nhất 2 tuần dữ liệu `runs` từ việc dùng thực tế)
**Requirements**: OPT-01, OPT-02, OPT-03, OPT-04, OPT-05, OPT-06, OPT-07
**Success Criteria** (what must be TRUE):
  1. Người dùng xem được báo cáo token trung bình theo vai trò và theo dự án, kèm token của 3 turn đầu mỗi run, với số liệu trước và sau tối ưu.
  2. Với `.gsd-summary.md`, Dev bắt đầu sửa code trong ≤ 2 turn; sửa `.gsd` thì summary tự được tạo lại ở lần chạy sau.
  3. Khi session vượt ngưỡng (ban đầu 45.000 token), có `.handover.md` và session mới tiếp tục đúng việc mà không hỏi lại những quyết định đã chốt; session cũ vẫn được giữ.
  4. Token/task trung bình giảm đo được so với mốc Phase 3 (cổng phase — không đóng phase khi chưa có số liệu này).
  5. Quyết định có làm golden session + fork (phương án B) hay không đã được ghi lại kèm lý do dựa trên số liệu đo lại.

**Plans**: TBD

### Phase 5: Embedded Dashboard

**Goal**: Người dùng xem task và log realtime, giao việc, kill và xem thống kê token qua web UI nằm trong chính binary `agent-brain`, vẫn là một file chạy duy nhất.
**Source**: Phase 4 — `05-phase4-dashboard.md`
**Depends on**: Phase 3
**Requirements**: DASH-01, DASH-02, DASH-03, DASH-04, DASH-05, DASH-06, DASH-07, DASH-08, DASH-09, DASH-10, DASH-11, DASH-12
**Success Criteria** (what must be TRUE):
  1. Chạy binary thì mở được dashboard mà không cần cài thêm gì, giao diện vẫn đúng style khi tắt internet, máy khác trong LAN không truy cập được và truy cập không có token bị từ chối.
  2. Giao việc từ UI bằng yêu cầu tiếng Việt thì log hiện realtime và từng bước pipeline Lead → Dev → QA đổi trạng thái; tên dự án mơ hồ hiện danh sách để chọn thay vì tự chạy.
  3. Reload trang giữa một run thì vẫn thấy log cũ và tiếp tục nhận log mới.
  4. Bấm Kill trên UI thì run dừng và không còn tiến trình sót.
  5. Đổi profile thì thống kê và task mới theo đúng profile; màn hình Thống kê (token theo ngày/profile/dự án/vai trò) và màn hình Dự án (sửa alias, quét lại) hoạt động; nếu có thiết kế Figma thì UI khớp checklist UI của người dùng.

**Plans**: TBD
**UI hint**: yes

### Phase 6: Multi-engine & Media Branch

**Goal**: Mỗi vai trò (Lead/Dev/QA) chạy được bằng engine khác nhau và đổi được giữa các task khi hết quota (không đổi ngầm), và sản xuất video chạy ở nhánh riêng tách khỏi Git worktree nhưng vẫn điều phối chung một nơi.
**Source**: Phase 5 — `06-phase5-da-doi-hinh-media.md`
**Depends on**: Phase 3 (kết quả S1, S5, S6, S7 từ Phase 1; phụ thuộc mềm vào `.handover.md` của Phase 4)
**Requirements**: ENG-01, ENG-02, ENG-03, ENG-04, ENG-05, ENG-06, ENG-07, MEDIA-01, MEDIA-02, MEDIA-03
**Success Criteria** (what must be TRUE):
  1. Chạy task với đội hình `tiet-kiem`: Lead và QA chạy Claude, Dev chạy DeepSeek, log ghi đúng engine và thống kê tách đúng token từng engine (cổng phase: 1 task trộn engine chạy end-to-end).
  2. Khi hết quota giữa task, hệ thống báo lỗi và đề xuất đổi đội hình, không tự đổi ngầm; đổi engine giữa task thì engine mới tiếp tục từ `.plan.md` (và `.handover.md` nếu có) mà không làm lại từ đầu.
  3. Key DeepSeek không xuất hiện trong file đã commit hay trong log.
  4. Dự án media không tạo Git worktree, file render nằm ngoài repo; kịch bản whiteboard đã duyệt chuyển thành cấu hình render rồi file output; kill job render thì FFmpeg dừng và không còn tiến trình sót.
  5. Antigravity được xử lý theo kết quả S7 (chạy headless như engine, hoặc dùng thủ công và theo dõi qua `git diff`), và tỉ lệ "Cần sửa" của QA được ghi theo engine.

**Plans**: TBD

## Cross-cutting KPIs

Các KPI áp dụng xuyên suốt các phase (nguồn `00-ROADMAP.md` mục 5); kiểm tra ở các phase gắn kèm:

| Chỉ số | Cách đo | Mục tiêu | Kiểm ở |
|--------|---------|----------|--------|
| Token trung bình / task | SQLite (từ Phase 3) | Giảm dần qua các phase | Phase 3 (mốc), Phase 4 (OPT-06) |
| Số turn trước khi agent bắt đầu sửa code | Đếm trong log stream-json | ≤ 2 | Phase 2 (nhật ký), Phase 4 (OPT-02) |
| Task chạy nhầm repo | Ghi tay / log | 0 | Phase 2 (ROUTE-03, ROUTE-04), Phase 5 (DASH-07) |
| Tiến trình còn sót sau Kill | `ps` sau khi kill | 0 | Phase 3 (PROC-01, PROC-02), Phase 5 (DASH-06), Phase 6 (MEDIA-02) |
| Task phải làm lại do Dev hiểu sai plan | Ghi tay (nhật ký task) | Giảm dần | Phase 2 (ROLE-07), Phase 6 (ENG-07) |

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 (Phase 4, 5, 6 độc lập sau Phase 3; thứ tự thực tế chọn sau Phase 3 — xem Execution Order & Dependencies ở trên)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Verify Assumptions | 1/1 | In Progress | - |
| 2. Protocol MVP (Scripts) | 0/TBD | Not started | - |
| 3. Rust CLI Core | 0/TBD | Not started | - |
| 4. Session & Token Optimization | 0/TBD | Not started | - |
| 5. Embedded Dashboard | 0/TBD | Not started | - |
| 6. Multi-engine & Media Branch | 0/TBD | Not started | - |
