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

- [x] **Phase 1: Verify Assumptions** - Thử thật 8 spike (S1–S8) và ghi `spike-results.md` trước khi viết dòng Rust nào (completed 2026-10-15)
- [x] **Phase 2: Protocol MVP (Scripts)** - Chạy Lead → `.plan.md` → Dev → QA trên dự án thật chỉ bằng script và file cấu hình (completed 2026-10-16)
- [x] **Phase 3: Rust CLI Core** - Binary `agent-brain` với kill sạch cây tiến trình, bắt log stream-json, token trong SQLite, fast-path (completed 2026-10-16)
- [ ] **Phase 4: Session & Token Optimization** - Giảm token/task dựa trên số liệu `runs` thật (`.gsd-summary.md`, `.handover.md`)
- [x] **Phase 5: Embedded Dashboard** - Web UI + SSE trong cùng binary: xem log realtime, giao việc, kill, thống kê (completed 2026-10-19)
- [ ] **Phase 6: Multi-engine & Media Branch** - Mỗi vai trò một engine (claude, DeepSeek, agy, nhân sự), đội hình + màn Cài đặt, xử lý hết quota không đổi ngầm (nhánh Media tách sang 06.1)
- [ ] **Phase 06.1: Media Branch (INSERTED)** - Dự án media chạy ngoài Git worktree, job render FFmpeg có kill sạch, bảng `media_jobs`

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

**Plans**: 7/7 plans complete

Plans:
**Wave 1**
- [x] 02-01-PLAN.md — Tracer: `brain lead` end to end against the fake claude (routing, profile env, task branch, runner, `.plan.md` commit) — wave 1

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 02-02-PLAN.md — `.plan.md` / `.qa-report.md` file contract: spec templates, parser/validator, JSON fixtures shared with Phase 3 — wave 2

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 02-03-PLAN.md — Lead protocol enforcement: skeleton, ROLE-01 post-run check (P1-06), plan validation, refusals, re-plan, Lead prompt rules — wave 3

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 02-04-PLAN.md — Project selection: P1-01..P1-04 + P1-10 from shared JSON fixtures, `brain scan` / `brain add` (ROUTE-05/06) — wave 4
- [x] 02-05-PLAN.md — Dev + QA roles: guard hook, `brain dev` (Vướng mắc stop), `brain qa` (report only), lead → dev → qa pipeline — wave 4
- [x] 02-06-PLAN.md — Task log instrumentation (stream-json turn counting, spec §7 log, 5-task check) + installer, README, live manual — wave 4

**Wave 5** *(blocked on Wave 4 completion)*
- [x] 02-07-PLAN.md — Integration preflight + live acceptance P1-01..P1-10 + 5 real tasks (user checkpoints) — wave 5

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

**Plans**: 13/13 plans complete

Plans:
**Wave 1**
- [x] 03-01-PLAN.md — Crate manifest with pinned deps, error module, embedded prompts/templates/config; user checkpoints: crate legitimacy, musl toolchain + lockfile + fetch
- [x] 03-02-PLAN.md — Shared contract fixtures `tests/contract/stream.json` and `guard.json` with Python loaders (D-03)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 03-03-PLAN.md — Foundation: config, Vietnamese routing, project map, profile env, git ops, plan meta, SQLite schema v1 + API, task adoption, /proc identity, stale-run reconciliation (library tests, no binary yet)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 03-04-PLAN.md — Tracer: `agent-brain run lead` end to end (CLI → routing → profile → git → runner → stream → SQLite → terminal), command entry points, refusal paths, test sandbox

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 03-05-PLAN.md — `projects scan/add`, routing, profile and config parity on the shared fixtures (CLI-02, PROF-02)
- [x] 03-06-PLAN.md — `.plan.md` / `.qa-report.md` contract and prompt/template byte parity (CLI-03)
- [x] 03-07-PLAN.md — Stream summary parity + partial usage, terminal lines, P2-04/P2-05, `agent-brain guard` hook (LOG-01/02)
- [x] 03-08-PLAN.md — `status` (0 token), `inspect` (1 turn, no session), `stats` (FAST-01/02, STAT-01)

**Wave 5** *(blocked on Wave 4 completion)*
- [x] 03-09-PLAN.md — S4 process-tree kill, Ctrl+C/SIGHUP/timeout handling, `kill <task_id>`, PID-reuse safety (PROC-01/02)
- [x] 03-10-PLAN.md — Lead parity through the binary: plan validation, re-plan, branch rules, dry-run, E-06 personal-run refusal, shared role helpers (CLI-03)

**Wave 6** *(blocked on Wave 5 completion)*
- [x] 03-11-PLAN.md — Dev and QA parity through the binary and the lead → dev → qa pipeline (CLI-03)

**Wave 7** *(blocked on Wave 6 completion)*
- [x] 03-12-PLAN.md — Static musl release build + DIST-01 check, README, live-check script and wrapper

**Wave 8** *(blocked on Wave 7 completion)*
- [x] 03-13-PLAN.md — Live acceptance P2-01/02/03/07/09, D-02 parity task, E-06 personal run, retire Python symlink (user checkpoints)

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

**Plans**: 13/13 plans complete
**UI hint**: yes

Plans:
**Wave 1**
- [x] 05-01-PLAN.md — Pin axum/futures-util, offline CSS tooling; user checkpoints: legitimacy (Alpine CSP flagged SUS), cargo fetch + Cargo.lock, Tailwind binary, Alpine vendoring, first app.css

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05-02-PLAN.md — Tracer: `agent-brain serve` → token gate → embedded shell → live SSE of a terminal-started run

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 05-03-PLAN.md — Serve hardening: single-instance lock, port errors, loopback-only proof, refusal matrix and headers, Last-Event-ID resume, shutdown with an open stream

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 05-04-PLAN.md — Giao việc server: job manager (child `agent-brain run`, PDEATHSIG), POST /api/tasks, preflight, busy rules, resolve gate, D-08 stop/crash

**Wave 5** *(blocked on Wave 4 completion)*
- [x] 05-05-PLAN.md — Task pipeline server: derived step states/actions, task list/detail with plan & QA, Dev/QA/re-plan, Kill, Lead outcome fix
- [x] 05-06-PLAN.md — Stats server: by day / by profile breakdowns, GET /api/stats with profile & since filters
- [x] 05-07-PLAN.md — Projects server: ranked suggestions, list/status, alias edit with etag, rescan

**Wave 6** *(blocked on Wave 5 completion)*
- [x] 05-08-PLAN.md — UI shell (router, profile switcher, theme) + Tổng quan screen + headless smoke script

**Wave 7** *(blocked on Wave 6 completion)*
- [x] 05-09-PLAN.md — UI Giao việc screen: project autocomplete, editable slug, preflight reasons, start and follow the job

**Wave 8** *(blocked on Wave 7 completion)*
- [x] 05-10-PLAN.md — UI Chi tiết task: stepper, actions/Kill, read-only plan & QA, live log pane

**Wave 9** *(blocked on Wave 8 completion)*
- [x] 05-11-PLAN.md — UI Thống kê + Dự án screens; DASH-09 offline asset test; five-screen smoke

**Wave 10** *(blocked on Wave 9 completion)*
- [x] 05-12-PLAN.md — Release smoke (serve from the static binary in env -i), README, LIVE-CHECKS P4, acceptance table

**Wave 11** *(blocked on Wave 10 completion)*
- [x] 05-13-PLAN.md — Live acceptance P4-01..P4-10 with the real claude, browser, LAN and offline checks (user checkpoints)

### Phase 05.1: Pipeline View (INSERTED)

**Goal**: Màn hình Chi tiết task hiển thị pipeline kiểu "Autopilot" — thẻ giai đoạn có vòng, thời gian, chi phí, cổng duyệt "Bạn" và sổ lượt chạy kèm tổng — dựng hoàn toàn từ dữ liệu run hiện có (bước A của Dashboard v2).
**Source**: Thiết kế Dashboard v2 (https://claude.ai/artifact/KdMc5CMNSMg4G5T9jLjFRK), quyết định người dùng 2026-10-19
**Depends on**: Phase 5
**Requirements**: PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05
**Success Criteria** (what must be TRUE):
  1. Mở một task đã chạy Lead → Dev → QA (kể cả có chạy lại) thì thấy các thẻ giai đoạn đúng trạng thái, đúng số vòng, thời gian và chi phí khớp `brain.db`.
  2. Sổ lượt chạy liệt kê đủ mọi run của task theo thứ tự với lượt · phút · $, và dòng tổng bằng tổng các run.
  3. Thẻ "Bạn" chuyển "chờ bạn" → "đã duyệt" đúng lúc Dev chạy trên plan hiện tại; giai đoạn chưa có hiện "chưa tự động", không giả lập.
  4. Khi một run đang chạy, thẻ và sổ cập nhật trực tiếp; Kill và các nút bước vẫn hoạt động như Phase 5.
  5. Màn hình đạt ở 1024/768/480 px, sáng/tối, bàn phím; không đổi schema CSDL hay giao thức vai trò.

**UI hint**: yes
**Plans:** 5/5 plans complete

Plans:
**Wave 1**
- [x] 05.1-01-PLAN.md — Tracer: `GET /api/tasks/{id}/pipeline` (runs_for_task → pure pipeline::derive: cards, rounds, cycles, ledger, totals) + rule battery and brain.db flow tests
- [x] 05.1-02-PLAN.md — Shared status vocabulary (đang chạy blue, D-08 glyphs, state maps), formatters fmtClock/fmtMinutes/fmtUsd, focusLater fallback, node vm JS check

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05.1-03-PLAN.md — Git-backed "Bạn" gate: plan-commit ancestry of Dev base_commit, single plan-commit lookup, run-order fallback
- [x] 05.1-04-PLAN.md — Task screen pipeline card: five stage cards, loop groups, chưa tự động chips, action strip, legend, live refetch, dialog focus return

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 05.1-05-PLAN.md — Run ledger (cycle headers, row → log, total, Hiện thêm 20), CDP keyboard + 1024/768/480 layout checks, app.css rebuild, phase gate

### Phase 05.2: Workflows and Autopilot (INSERTED)

**Goal**: Mỗi task chạy theo một quy trình (workflow) có tên — mặc định `mac-dinh` — với hai vai trò review mới (review plan, review code) và chế độ sửa sau QA; autopilot tự chạy các vòng lặp tới khi đạt hoặc chạm giới hạn (3 vòng, 60 phút hoặc $20), luôn dừng ở cổng duyệt plan của người dùng, và dashboard hiện các giai đoạn này như thẻ thật (bước B của Dashboard v2).
**Source**: Thiết kế Dashboard v2 (https://claude.ai/artifact/KdMc5CMNSMg4G5T9jLjFRK), quyết định người dùng 2026-10-19
**Depends on**: Phase 05.1
**Requirements**: AUTO-01, AUTO-02, AUTO-03, AUTO-04, AUTO-05, AUTO-06, AUTO-07
**Success Criteria** (what must be TRUE):
  1. `agent-brain run plan-review` / `code-review` chạy được trên task, chỉ ghi file báo cáo của mình, và báo cáo có verdict + số chặn/góp ý đọc được bằng máy.
  2. Giao việc chọn được quy trình theo tên (CLI `--workflow`, dashboard "Quy trình"); không chọn thì dùng `mac-dinh`; task lưu bản sao quy trình lúc bắt đầu.
  3. Autopilot chạy plan ↔ review plan tới khi duyệt hoặc hết vòng, dừng chờ người dùng duyệt plan, rồi chạy code ↔ review code và QA ↔ sửa; dừng đúng khi hết vòng, hết thời gian/tiền, vi phạm giao thức hoặc vướng mắc, kèm lý do.
  4. Kill trong lúc autopilot chạy dừng cả driver lẫn run hiện tại, không sót tiến trình; các quy tắc một run/dự án và một run personal vẫn giữ.
  5. Dashboard hiện thẻ review/sửa thật (vòng N / giới hạn, số chặn/góp ý), nút duyệt plan, bật/tắt autopilot; CSDL cũ nâng cấp lên schema mới không mất dữ liệu.

**UI hint**: yes
**Plans:** 32/32 plans complete

Plans:
- [x] 05.2-32-PLAN.md
- [x] 05.2-21-PLAN.md
- [x] 05.2-22-PLAN.md
- [x] 05.2-23-PLAN.md
- [x] 05.2-24-PLAN.md
- [x] 05.2-25-PLAN.md
- [x] 05.2-26-PLAN.md
- [x] 05.2-27-PLAN.md
- [x] 05.2-28-PLAN.md
- [x] 05.2-29-PLAN.md
- [x] 05.2-30-PLAN.md
- [x] 05.2-31-PLAN.md

**Wave 1**
- [x] 05.2-01-PLAN.md — Tracer: schema v2 (runs rebuild with FK-safe runner, tasks columns incl. approved_commit) proven on a v1 DB with events; v2 data helpers; module stubs
- [x] 05.2-02-PLAN.md — Config defaults (review models/timeouts, autopilot limits, price table) and test harness (fake claude per-role queue, Sandbox workflow helpers)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05.2-03-PLAN.md — Workflows (built-ins, workflows.json validation with Vietnamese key/line errors, resolution, snapshot) and pricing/budget arithmetic
- [x] 05.2-04-PLAN.md — Report front matter parser, named files for fix mode, shared protocol-file list, tri-state ancestry and bounded plan-commit lookup (05.1 WR-01/WR-02 helpers)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 05.2-05-PLAN.md — Review roles `run plan-review|code-review` (guarded report-only writes, D-06 verdicts, staged run recording)
- [x] 05.2-06-PLAN.md — Autopilot pure core: decide() rules (loops, caps, gate, budget, stops, resume) and stop wording
- [x] 05.2-13-PLAN.md — GET /api/workflows and per-project workflow on project JSON

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 05.2-07-PLAN.md — Dev fix mode (`--fix qa|code-review`), QA counts, protocol-aware QA diff, implicit approval
- [x] 05.2-08-PLAN.md — Task state derivation: review words, autopilot state, controls, any-role Kill, driver-aware actions

**Wave 5** *(blocked on Wave 4 completion)*
- [x] 05.2-09-PLAN.md — Lead: workflow snapshot (`--workflow`), `--review` / `--note=` re-plans on the committed plan, approval clearing
- [x] 05.2-10-PLAN.md — Pipeline flow derivation: real review/fix cards, footers with cap, gate view and notes, budget, staged ledger (05.1 WR-02/WR-03)

**Wave 6** *(blocked on Wave 5 completion)*
- [x] 05.2-11-PLAN.md — Autopilot driver `agent-brain auto` / `auto --start` / `approve` (claim, child stages, stops, signals, kill flag)

**Wave 7** *(blocked on Wave 6 completion)*
- [x] 05.2-12-PLAN.md — Kill stops the autopilot through runner::cmd_kill; kill mid-loop and in the gap leave no process
- [x] 05.2-14-PLAN.md — Task API read side: pipeline flow over HTTP, controls/auto/rev, driver-aware busy rules, Giao việc workflow (05.1 WR-01, IN-05)

**Wave 8** *(blocked on Wave 7 completion)*
- [x] 05.2-15-PLAN.md — Action endpoints: approve, return (note), auto start/stop/more-round/end, stale and duplicate 409s, serve shutdown (D-16)

**Wave 9** *(blocked on Wave 8 completion)*
- [x] 05.2-16-PLAN.md — Task screen: data-driven flow with review/fix cards, footers, gate note, autopilot badge/toggle, budget line (05.1 WR-04)

**Wave 10** *(blocked on Wave 9 completion)*
- [x] 05.2-17-PLAN.md — Task screen: Duyệt plan / Trả về Lead (note dialog), stop banner and actions, ledger counts and token line

**Wave 11** *(blocked on Wave 10 completion)*
- [x] 05.2-18-PLAN.md — Giao việc Quy trình picker, Dự án workflow list, Tổng quan reason line and Autopilot badge

**Wave 12** *(blocked on Wave 11 completion)*
- [x] 05.2-19-PLAN.md — Keyboard/layout checks, app.css rebuild, README, LIVE-CHECKS Phase 5.2 procedure, results template (05.1 WR-05)

**Wave 13** *(blocked on Wave 12 completion)*
- [x] 05.2-20-PLAN.md — AUTO-07 live checks with the real claude in an isolated environment (user go/no-go checkpoint)

### Phase 05.3: Telegram Notifications and Actions (INSERTED)

**Goal**: Người dùng nhận tin Telegram khi task cần mình (chờ duyệt plan, autopilot dừng kèm lý do, QA đạt, run lỗi) và xử lý ngay trong tin nhắn bằng nút Duyệt plan, Trả về Lead (kèm ghi chú), Chạy thêm 1 vòng, Dừng hẳn, Kill (xác nhận lại) — bot tự hỏi Telegram (long polling), không mở cổng vào máy, chỉ chat ID được phép; không giao việc mới qua Telegram.
**Source**: Quyết định người dùng 2026-10-20 (đặt sau 5.2, trước bảng công việc; phạm vi: thông báo + hành động ở cổng/khi dừng)
**Depends on**: Phase 05.2
**Requirements**: TELE-01, TELE-02, TELE-03, TELE-04, TELE-05, TELE-06, TELE-07
**Notes**: Mở lại một quyết định Out of Scope (truy cập từ xa) theo cách hẹp — xem PROJECT.md. Gọi Telegram qua `curl` của hệ thống (05.3-CONTEXT D-04), không thêm crate HTTPS.
**UI hint**: no
**Plans:** 9/9 plans complete

Plans:
**Wave 1**
- [x] 05.3-01-PLAN.md — Bot connects inside serve: telegram.json (0600), curl `-K -` transport, status line, fake Telegram (wave 1)
- [x] 05.3-02-PLAN.md — Shared action layer: run_control / run_task_action / kill_run extracted, dashboard responses unchanged (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05.3-03-PLAN.md — Notifications: gate (with plan document), stop, QA đạt, run lỗi via pump hints; once per event (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 05.3-04-PLAN.md — Outage resilience (D-05): queue, backoff 5→60 s, 429, drop stale, one warning line per outage; outbox edit items; two tasks at once (wave 3)
- [x] 05.3-05-PLAN.md — Buttons act: long poll, chat-ID allowlist, Duyệt plan / Chạy thêm 1 vòng / Dừng hẳn, stale handling (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 05.3-06-PLAN.md — Trả về Lead note prompt, Kill with confirmation, edits that show who acted (wave 4)
- [x] 05.3-07-PLAN.md — Pairing CLI: `agent-brain telegram setup` / `test` (wave 4)

**Wave 5** *(blocked on Wave 4 completion)*
- [x] 05.3-08-PLAN.md — Docs + live check with the user's bot and phone (user checkpoints) (wave 5)

**Wave 6** *(gap closure, blocked on Wave 5 completion)*
- [x] 05.3-09-PLAN.md — G-01: Chạy thêm 1 vòng after Trả về Lead past the cap runs exactly one more round (wave 6)

### Phase 05.4: Telegram Task Intake (INSERTED)

**Goal**: Người dùng giao việc từ Telegram: lệnh `/task` (nhập yêu cầu → chọn dự án → chọn quy trình → xác nhận) và chuyển tiếp tin nhắn từ chat khác vào bot (gom tin đến trong một khoảng ngắn, chọn các tin cần bằng nút ☑/☐, gộp thành **một** task, trích dẫn kèm người gửi và thời gian) — dùng cùng hàm tạo task và preflight với màn Giao việc.
**Source**: Quyết định người dùng 2026-10-21 (phase riêng sau 5.3; tin chuyển tiếp gộp thành 1 task; plan của task tạo từ Telegram chỉ duyệt trên dashboard)
**Depends on**: Phase 05.3
**Requirements**: INTAKE-01, INTAKE-02, INTAKE-03, INTAKE-04, INTAKE-05, INTAKE-06, INTAKE-07, INTAKE-08, INTAKE-09, INTAKE-10, INTAKE-11, INTAKE-12 (đề xuất từ 05.4-RESEARCH.md; đăng ký vào REQUIREMENTS.md ở plan 05.4-01 Task 3)
**Notes**: Mở rộng ngoại lệ Telegram trong PROJECT.md. Ràng buộc an toàn đã chốt: plan của task tạo từ Telegram **chỉ duyệt trên dashboard** (tin cổng duyệt của các task này không có nút Duyệt plan). Đề xuất cho discuss: danh sách dự án cho phép (`telegram_projects`), giới hạn số task/ngày, nội dung chuyển tiếp được đánh dấu là dữ liệu không tin cậy trong prompt Lead và ghi nguồn trong plan. Tên lệnh Telegram chỉ gồm chữ thường, số, `_` (không dùng `/create-task`). Ảnh/tệp trong tin chuyển tiếp: làm sau.
**UI hint**: no
**Plans:** 8/8 plans complete

Plans:
**Wave 1**
- [x] 05.4-01-PLAN.md — Shared `start_new_task` extracted from Giao việc (dashboard responses unchanged); INTAKE-01..12 registered in REQUIREMENTS.md (wave 1)
- [x] 05.4-02-PLAN.md — Durable `tasks.origin` (schema v3, decision gate), hidden `--origin telegram`, daily count (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05.4-03-PLAN.md — Dashboard-only approval: no Duyệt plan button for Telegram tasks, forged approve refused (wave 2)
- [x] 05.4-04-PLAN.md — Forwarded text as untrusted data: nonce-delimited block, Lead rules from origin, source tag in plan title (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 05.4-05-PLAN.md — `/task` flow: request → allowlisted project → workflow → confirm; cancel/expiry; callback grammar (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 05.4-06-PLAN.md — Outcome messages, daily cap, `/task` + `/huy` command menu (wave 4)

**Wave 5** *(blocked on Wave 4 completion)*
- [x] 05.4-07-PLAN.md — Forwards: collection window, ☑/☐ selection, one task quoting sender and time (wave 5)

**Wave 6** *(blocked on Wave 5 completion)*
- [x] 05.4-08-PLAN.md — Docs, PROJECT.md exception, live check with the user's bot and phone (user checkpoints) (wave 6)

### Phase 05.5: Work Board (INSERTED)

**Goal**: Màn hình mới `#/board` (View 2 của Dashboard v2): các hạng mục công việc xếp theo wave (wave 0 không phụ thuộc, wave sau chỉ phụ thuộc wave trước), đường nối phụ thuộc, đường găng (critical path) tô nổi, bấm một hạng mục để làm sáng những gì nó chờ và những gì chờ nó, công tắc ẩn/thu gọn hạng mục đã xong — dữ liệu nhập từ `ROADMAP.md` + front matter plan của GSD (`wave`, `depends_on`) trước, chế độ epic của Lead làm sau.
**Source**: Thiết kế Dashboard v2 (`05.1-DESIGN-REF.html`, "View 2 · Work board"; https://claude.ai/artifact/KdMc5CMNSMg4G5T9jLjFRK), quyết định người dùng 2026-10-19 (Phase C, nhập roadmap GSD trước) và 2026-10-20 (bảng công việc → 5.4), 2026-10-21 (nhận việc qua Telegram chen vào 5.4, bảng → 5.5)
**Depends on**: Phase 05.4
**Requirements**: BOARD-01, BOARD-02, BOARD-03, BOARD-04, BOARD-05, BOARD-06, BOARD-07, BOARD-08, BOARD-09, BOARD-10, BOARD-11, BOARD-12, BOARD-13, BOARD-14
**Notes**: Phase này chỉ làm nửa đầu của bước C: bảng chỉ đọc, đọc trực tiếp `ROADMAP.md`, `STATE.md` và front matter plan GSD trong `.planning/` của dự án mỗi lần gọi `GET /api/board?project=&epic=&milestone=` (không bảng `epics`, không migration — 05.5-CONTEXT D-05); bảng `epics`/hạng mục/phụ thuộc, chế độ epic của Lead, sửa tay và "bắt đầu hạng mục thành task" làm ở phase sau, giữ cùng dạng phản hồi. Đường găng chỉ tính hạng mục chưa xong (quyết định 2026-10-23). Giữ nguyên quy tắc Phase 5: `127.0.0.1` + token, Tailwind v4 build, Alpine CSP, không CDN, offline, tiếng Việt, sáng/tối, `dashboard-ui-lint.sh` phải qua.
**UI hint**: yes
**Plans:** 6/6 plans complete

Plans:
**Wave 1**
- [x] 05.5-01-PLAN.md — Tracer: live read of `.planning/` → token-gated GET-only `/api/board`, JSON contract, empty states, default epic; BOARD-01..14 registered in REQUIREMENTS.md (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 05.5-02-PLAN.md — `#/board` screen: waves, cards, per-edge SVG, "Thu gọn mục đã xong", filters, Đọc lại, warnings box, nav, app.css (wave 2)
- [x] 05.5-03-PLAN.md — Derive rules in `board.rs`: trimmed critical path, warnings + cycle break, wave mismatch, phase links, milestones, caps (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 05.5-04-PLAN.md — Scanner: roadmap parsing, hostile-repo tests (symlinks, FIFO, caps), broken-data warnings over HTTP (wave 3)
- [x] 05.5-05-PLAN.md — Focus mode, read-only side panel, Enter/Space/Esc in headless Chrome, layout at 1024/768/480 (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 05.5-06-PLAN.md — Live check on this repo + SHOP demo, README, LIVE-CHECKS §7, full regression, browser check (wave 4)

### Phase 6: Multi-engine & Media Branch

**Goal**: Mỗi vai trò (Lead/Dev/QA và hai vai trò review) chạy được bằng engine khác nhau (claude theo profile dự án, DeepSeek, Antigravity, nhân sự) qua đội hình đặt tên, cấu hình trên màn Cài đặt; hết quota thì dừng chờ người dùng và đổi được đội hình, không đổi ngầm. (Nhánh Media chuyển sang Phase 06.1.)
**Source**: Phase 5 — `06-phase5-da-doi-hinh-media.md`
**Depends on**: Phase 3 (kết quả S1, S5, S6, S7 từ Phase 1; phụ thuộc mềm vào `.handover.md` của Phase 4)
**Requirements**: ENG-01, ENG-02, ENG-03, ENG-04, ENG-05, ENG-06, ENG-07 (+ ID mới cho màn Cài đặt và engine `người`, planner đăng ký vào REQUIREMENTS.md); MEDIA-01…03 chuyển sang Phase 06.1 (06-CONTEXT D-22, quyết định người dùng 2026-10-24)
**Success Criteria** (what must be TRUE):
  1. Chạy task với đội hình `tiet-kiem`: Lead và QA chạy Claude, Dev chạy DeepSeek, log ghi đúng engine và thống kê tách đúng token từng engine (cổng phase: 1 task trộn engine chạy end-to-end).
  2. Khi hết quota giữa task, hệ thống báo lỗi và đề xuất đổi đội hình, không tự đổi ngầm; đổi engine giữa task thì engine mới tiếp tục từ `.plan.md` (và `.handover.md` nếu có) mà không làm lại từ đầu.
  3. Key DeepSeek không xuất hiện trong file đã commit hay trong log.
  4. *(chuyển sang Phase 06.1)*
  5. Antigravity được xử lý theo kết quả S7 (chạy headless như engine, hoặc dùng thủ công và theo dõi qua `git diff`), và tỉ lệ "Cần sửa" của QA được ghi theo engine.

**Plans:** 14/19 plans executed

Plans:
**Wave 1**
- [x] 06-01-PLAN.md — E-09 decision gate: one tiny real DeepSeek call via `claude` (user-run, D-02) (wave 1)
- [x] 06-02-PLAN.md — agy 1.4.7 probe: event shape, narrow Dev permission mode (user-run, D-26) (wave 1)
- [x] 06-03-PLAN.md — Engine catalog, `teams.json`/`staff.json` loaders and rules; register TEAM/SET/HUM/QUOTA IDs (wave 1)
- [x] 06-04-PLAN.md — Schema v4 `tasks.team_json` + run engine recording (blocking decision gate, one-way) (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 06-05-PLAN.md — One engine seam per role run: `runs.engine`, header `engine …`, `--team` (wave 2)
- [x] 06-06-PLAN.md — Settings server API: atomic writes, version token, engine readiness (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 06-07-PLAN.md — Quota stop: outcome `quota`, exit 5, autopilot `hết quota`, continue on existing diff (wave 3)
- [x] 06-08-PLAN.md — Cài đặt screen (`#/settings`) per UI-SPEC (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 06-09-PLAN.md — agy as a headless engine: parser, stdout-to-file, Dev diff scope check (wave 4)
- [x] 06-10-PLAN.md — Task team operations: pick, change with history, quota re-run (shared actions) (wave 4)

**Wave 5** *(blocked on Wave 4 completion)*
- [ ] 06-11-PLAN.md — DeepSeek engine + key hygiene (only if E-09 passed) (wave 5)
- [ ] 06-12-PLAN.md — Human engine CLI side: prepare/push branch, wait marker, Xong/Trả lại (wave 5)
- [x] 06-13-PLAN.md — Engines on the pipeline, per-engine stats, Cần sửa by Dev engine (wave 5)
- [x] 06-14-PLAN.md — Telegram quota message with team buttons + confirm tap (wave 5)
- [x] 06-15-PLAN.md — Team picker on Giao việc (wave 5)

**Wave 6** *(blocked on Wave 5 completion)*
- [ ] 06-16-PLAN.md — Human engine on the dashboard (server side) (wave 6)
- [x] 06-17-PLAN.md — Task screen: team line, engines per run, quota banner (wave 6)

**Wave 7** *(blocked on Wave 6 completion)*
- [ ] 06-18-PLAN.md — Human callout, Tổng quan and Thống kê screens (wave 7)

**Wave 8** *(blocked on Wave 7 completion)*
- [ ] 06-19-PLAN.md — Live check, schema v4 dry run on a copy of brain.db, README, LIVE-CHECKS § 8, phase gate (wave 8)

### Phase 06.1: Media Branch (INSERTED)

**Goal**: Sản xuất video chạy ở nhánh riêng tách khỏi Git worktree nhưng vẫn điều phối chung một nơi: kịch bản đã duyệt → agent viết cấu hình/script render → runner chạy FFmpeg → file output.
**Source**: Phase 5 — `06-phase5-da-doi-hinh-media.md` §2.4, §3.2, §3.3 (tách khỏi Phase 6 theo 06-CONTEXT D-22)
**Depends on**: Phase 6 (đội hình/engine cho bước viết cấu hình render)
**Requirements**: MEDIA-01, MEDIA-02, MEDIA-03
**Success Criteria** (what must be TRUE):
  1. Dự án `type: media` là thư mục thường, không tạo Git worktree; file output nằm ở `<dự án media>/output/`, không nằm trong repo code nào.
  2. Kịch bản đã duyệt trong dự án media được agent chuyển thành cấu hình/script render, runner chạy ra file output; mỗi job ghi vào bảng `media_jobs`.
  3. Kill một job render thì FFmpeg dừng và không còn tiến trình sót (cách kill S4).

**Notes**: Pipeline kịch bản whiteboard thật chưa có — phase này làm job render chung; nối pipeline thật làm sau. Discuss riêng trước khi plan.
**Plans**: TBD

## Cross-cutting KPIs

Các KPI áp dụng xuyên suốt các phase (nguồn `00-ROADMAP.md` mục 5); kiểm tra ở các phase gắn kèm:

| Chỉ số | Cách đo | Mục tiêu | Kiểm ở |
|--------|---------|----------|--------|
| Token trung bình / task | SQLite (từ Phase 3) | Giảm dần qua các phase | Phase 3 (mốc), Phase 4 (OPT-06) |
| Số turn trước khi agent bắt đầu sửa code | Đếm trong log stream-json | ≤ 2 | Phase 2 (nhật ký), Phase 4 (OPT-02) |
| Task chạy nhầm repo | Ghi tay / log | 0 | Phase 2 (ROUTE-03, ROUTE-04), Phase 5 (DASH-07) |
| Tiến trình còn sót sau Kill | `ps` sau khi kill | 0 | Phase 3 (PROC-01, PROC-02), Phase 5 (DASH-06), Phase 06.1 (MEDIA-02) |
| Task phải làm lại do Dev hiểu sai plan | Ghi tay (nhật ký task) | Giảm dần | Phase 2 (ROLE-07), Phase 6 (ENG-07) |

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 (Phase 4, 5, 6 độc lập sau Phase 3; thứ tự thực tế chọn sau Phase 3 — xem Execution Order & Dependencies ở trên)

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Verify Assumptions | 1/1 | Complete    | 2026-10-15 |
| 2. Protocol MVP (Scripts) | 7/7 | Complete    | 2026-10-16 |
| 3. Rust CLI Core | 13/13 | Complete    | 2026-10-16 |
| 4. Session & Token Optimization | 0/TBD | Not started | - |
| 5. Embedded Dashboard | 13/13 | Complete    | 2026-10-19 |
| 6. Multi-engine & Media Branch | 14/19 | In Progress | - |
