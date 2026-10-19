---
gsd_state_version: "1.0"
current_phase: "05.1"
current_phase_name: Pipeline View
status: Ready to plan (UI-SPEC next)
stopped_at: Phase 5 complete, ready to plan Phase 4
last_updated: "2026-10-19T09:02:43.086Z"
last_activity: 2026-10-19
last_activity_desc: Phase 5 complete, transitioned to Phase 4
state_head: 820f92e07799d6fffa2167e5ede1ae00e8470e02
progress:
  total_phases: 7
  completed_phases: 4
  total_plans: 34
  completed_plans: 34
  percent: 57
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-19)

**Core value:** Giao việc cho nhiều AI agent bằng giao thức Lead → `.plan.md` → Dev → QA một cách đáng tin cậy: đúng dự án, đúng vai trò, kill sạch cây tiến trình, đo được token.
**Current focus:** Phase 4 — Session & Token Optimization (hoặc Phase 6; xem Blockers)

## Current Position

Phase: 05.1 — Pipeline View (INSERTED)
Plan: Not started
Status: Ready to plan (UI-SPEC next)
Last activity: 2026-10-19 — Phase 5 complete, transitioned to Phase 4

Progress: [██████░░░░] 57% (4/6 phases complete, 34/34 planned plans done)

## Performance Metrics

**Velocity:**
- Total plans completed: 34
- Average duration: -
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 1 | - | - |
| 2 | 7 | - | - |
| 03 | 13 | - | - |
| 5 | 13 | - | - |

**Recent Trend:**
- Last 5 plans: -
- Trend: -

*Updated after each plan completion*
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 1h 48m | 8 tasks | 97 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table. Không có quyết định bị khóa (không có ADR trong bộ tài liệu nạp).

- [Init]: Ánh xạ phase nguồn → GSD lệch +1 (nguồn Phase 0 = GSD Phase 1…); ID test P1-xx…P5-xx giữ số phase nguồn.
- [Init]: 65 yêu cầu v1 = 42 từ test case nguồn + 23 derived (mục tiêu/công việc/cổng/mitigation rủi ro); FAST-03 giữ ngoài v1 chờ quyết định.
- [Init]: Phase 4/5/6 độc lập sau Phase 3, thứ tự chọn sau Phase 3 theo nỗi đau lớn nhất.
- [Phase 01]: SPK-10: Work = ~/.claude (company Team seat), Personal = ~/.claude-orca (personal Pro); company policy unconfirmed and waived by user — User answered "SPK-10 alway accept ignore condition"; two-profile mapping accepted. Personal is ~/.claude-orca, not ~/.claude-personal. S5 live test deferred.
- [Phase 01]: S4 kill method for Phase 3: record claude PID and descendants, SIGTERM claude PID, wait 5 s, sweep survivors (TERM then KILL after 2 s); SIGKILL orphans children — Verified clean in 3/3 runs (SIGTERM, SIGKILL+sweep, SIGTERM to PGID); kill -- -PGID did not leave survivors, contradicting E-05
- [Phase 01]: S1/S8 command shapes: stream-json needs --verbose; FAST-02 uses embedded diff with --tools "" --no-session-persistence --max-turns 1 — Observed: stream-json without --verbose exits 1; --max-turns 1 on review git diff ends error_max_turns with no result text
- [Phase 05]: Dashboard `agent-brain serve` chỉ nghe 127.0.0.1 + token mỗi lần khởi động; cookie không tách theo port là rủi ro chấp nhận AR-5-01 (CR-01 deferred)
- [Phase 05]: Kill từ UI chỉ đi qua `runner::cmd_kill`; job con `agent-brain run` có PDEATHSIG; live update bằng pump `PRAGMA data_version`
- [Phase 05]: agy chạy với `--dangerously-skip-permissions` (người dùng duyệt cho Phase 5); kiểm UI tự động bằng `scripts/dashboard-smoke.sh --keys` (Chrome headless + CDP)
- [Phase 01]: S6/S7 engine facts for Phase 6: dsh headless is text-only and needs in-container kill; agy is headless with JSON but no turn limit — Observed in S6 (json_lines=0, container process survives client kill) and S7 (--max-turns rejected, exit 2)

### Pending Todos

None yet.

### Blockers/Concerns

- [Phase 5 / 05-06 review] Test flaky có sẵn từ Phase 3: `contract_guard::guard_contract_cases` thỉnh thoảng panic `write stdin: BrokenPipe` (tests/contract_guard.rs:202) vì `agent-brain guard` thoát trước khi đọc hết stdin; chạy lại 8/8 Đạt. Sửa sau: bỏ qua EPIPE khi ghi stdin trong test hoặc cho `guard` đọc hết stdin.
- [Phase 3 / 03-13] claude 2.1.301 chặn `sleep N` foreground trong Bash tool ("Blocked: standalone sleep") → P2-02/P2-03 dùng `python3 -c 'import time; time.sleep(307)'`, wrapper `scripts/live/claude-allow-sleep.sh` cho phép `Bash(python3 *)` (fix plan `.planning/fixes/03-13-live-sleep-method.plan.md`, agy, commits 65386f2, 87f8e71, 00443c9; đã review). Shell của Bash tool chạy trong PGID/SID riêng — kill phải quét theo PID đã ghi, không chỉ process group.
- [Phase 3, mở cho phase sau] Ghi nhận nhỏ, chưa sửa: log `run` in nhiều dòng `system thinking_tokens` (nhiễu); claude tạo thư mục rỗng `~/.claude/session-env/<id>/` cho mỗi phiên kể cả `inspect` không lưu session.

- [Phase 1] Đã xử lý: SPK-10 — người dùng miễn điều kiện chính sách công ty ("SPK-10 alway accept ignore condition"); ánh xạ Work=`~/.claude`, Personal=`~/.claude-orca`. Chính sách vẫn chưa được xác nhận bằng nguồn nào.
- [Phase 1] Đã xử lý: S7 — agy chạy headless, có JSON/stream-json, KHÔNG có cờ giới hạn turn (ENG-06 tích hợp được, giới hạn bằng `--print-timeout` + kill ngoài).
- [Phase 1]: Công cụ scrub/scan của spike có lỗi tồn đọng (CR-01, WR-01…WR-10 trong `01-REVIEW.md`); username hệ điều hành còn trong 4 fixture S1/S4 — re-scrub trước khi push; sửa bằng `/gsd-code-review 01 --fix` (theo CLAUDE.md: giao agy).
- [Phase 1] Đã xử lý ở Phase 3 (E-06 Đạt 2026-10-16): S5 chỉ ánh xạ, chưa chạy thử trực tiếp profile cá nhân (transcript tách biệt, cross-resume, xoay token E-06 chưa kiểm); làm trước khi chốt PROF ở Phase 3.
- [Phase 4]: Cần ≥ 2 tuần dữ liệu `runs` thật từ binary Phase 3 trước khi bắt đầu; không đóng phase khi chưa có OPT-06.
- [Phase 4]: Khoảng trống phạm vi — warm reuse `--resume` (fast-path cấp 3) bị hoãn từ Phase 2 nguồn sang Phase 3 nguồn mà không có công việc/test (FAST-03, đang ở v2); quyết định ở `/gsd-discuss-phase 4`.
- [Phase 6]: Phụ thuộc mềm vào `.handover.md` của Phase 4 (ENG-03 chỉ cần `.plan.md`).
- [Open — cần quyết định]: Phase 5 xong (2026-10-19). Còn Phase 4 (token) và 6 (routing). Phase 4 cần ≥ 2 tuần dữ liệu `runs` thật (dữ liệu từ 2026-10-16, đủ khoảng 2026-10-30) — nếu chưa đủ thì làm Phase 6 trước.
- [Phase 5, follow-up không chặn]: 05-REVIEW-DISPOSITION.md còn 25 finding mở — backend WR-05 (kiểm personal-run không nguyên tử), WR-06, WR-11 (kill theo run hiện tại của task), WR-01 (`parse_since` tràn số), WR-13 (500 lộ lỗi nội bộ); frontend WR-03 (focus không trả về nút mở dialog), WR-04 (Enter combobox trong 200 ms), WR-09/10, mũi tên stepper tương phản thấp; WR-02 README liệt kê 4 route không tồn tại. Đề xuất 2 fix plan qua agy.
- [Phase 5, mở]: P4-02 nửa khác máy chưa kết luận (LAN không vào được máy này); UI-04 cần kiểm lại khi Thống kê có > 14 ngày dữ liệu.
- [Ops] `projects_map.json` có thêm dự án nháp `du-an-nhap-ca-nhan` (profile personal) cho E-06; 2 phiên claude interactive trên `~/.claude-orca` (loyalty, phu-gia) đã dừng — mở lại bằng `claude --resume d22b640e-d2bd-4d51-a6fb-e409b1c23c54` / `ce573fa2-07a1-4b39-806f-106ad36e947f`.
- [Open]: Tham chiếu ngoài bộ nạp chưa có tài liệu: "bảng kiến trúc gốc" #1–#11, định dạng `.gsd`, pipeline kịch bản whiteboard, Orca, Paperclip (không có yêu cầu di chuyển).
- [Open]: `.planning/config.json` chưa tồn tại khi tạo roadmap; đã giả định granularity "standard" và ID phase tuần tự (`sequential`).

### Roadmap Evolution

- Phase 05.1 inserted after Phase 5: Pipeline View — Dashboard v2 step A (autopilot-style task pipeline + run ledger from existing data)

## Deferred Items

Items acknowledged and deferred at milestone close, most recent first:

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| Requirement | FAST-03 warm reuse (`--resume`) | Cần quyết định (v2) | 2026-10-14 | v1 |
| Requirement | SESS-01 golden session + fork | Phụ thuộc OPT-07 | 2026-10-14 | v1 |

## Session Continuity

Last session: 2026-10-19
Stopped at: Phase 5 complete, ready to plan Phase 4
Resume file: None
