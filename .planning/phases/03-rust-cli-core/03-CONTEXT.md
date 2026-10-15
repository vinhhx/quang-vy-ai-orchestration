# Phase 3: Rust CLI Core - Context

**Gathered:** 2026-10-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Binary Rust `agent-brain` (CLI, chưa có UI) chạy được giao thức Lead → `.plan.md` → Dev → QA của Phase 2 (CLI-01..03, PROF-02, PROC-01..02, LOG-01..02, FAST-01..02, STAT-01, DIST-01): định tuyến dự án như script Phase 2, process runner trong process group riêng và kill sạch cả cây, parser stream-json "mềm", lưu `runs`/`events` vào SQLite, fast-path cấp 1 (`status`, 0 token) và cấp 2 (`inspect`, đúng 1 turn), `stats` theo profile, binary portable. Ngoài phạm vi: web server/UI/SSE (Phase 5), tối ưu session và warm reuse `--resume` (Phase 4 / FAST-03 v2), chạy song song nhiều task, đa engine (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Cách xây & bàn giao từ Phase 2
- **D-01:** Code Rust do **GSD execute-phase** viết (plan + executor, test theo từng module, commit nguyên tử) — **không** dogfood qua `brain lead/dev/qa` như spec §8 gợi ý. Giao thức Phase 2 tiếp tục được kiểm chứng trên các dự án khác của người dùng.
- **D-02:** Script Python `brain` (Phase 2) **giữ đến khi `agent-brain` đạt parity** (qua test fixture chung + 1 task thật chạy bằng binary), sau đó **bỏ symlink khỏi PATH**; code `brain/` giữ lại trong repo làm bản tham chiếu, không bảo trì song song.
- **D-03:** Parity (CLI-02, CLI-03) chứng minh bằng **bộ test case trung lập ngôn ngữ**: các ca định tuyến/quét/parse `.plan.md` (P1-01..P1-05, P1-10 và tương tự) đặt thành file JSON (input → kết quả mong đợi) trong repo; cả test Python lẫn test Rust cùng đọc một bộ file này, lệch hành vi là test đỏ. Phase 2 nên tạo bộ fixture này (hoặc Phase 3 trích ra từ test Phase 2 làm việc đầu tiên). — **Reversibility:** costly — định dạng fixture là hợp đồng giữa hai bộ test; đổi format phải sửa cả hai phía.
- **D-04:** DIST-01: build **static musl `x86_64-unknown-linux-musl`** (rusqlite `bundled`), chỉ Linux x86_64 (khớp Phase 1 D-08). aarch64, macOS, Windows ngoài phạm vi.

### Nơi lưu dữ liệu & danh tính task
- **D-05:** **`~/.config/agent-brain/projects_map.json` là nguồn sự thật** cho dự án (người dùng vẫn sửa tay alias/`name_vi`/`profile` như Phase 2; `agent-brain projects scan` ghi file này với cùng luật không ghi đè trường sửa tay + cảnh báo alias trùng). Bảng SQLite `projects` chỉ là **bản phản chiếu**, đồng bộ từ JSON mỗi lần chạy, để `tasks`/`runs` tham chiếu. — **Reversibility:** costly — đảo chiều (SQLite làm chủ) cần migrate và bỏ thói quen sửa tay.
- **D-06:** CSDL SQLite ở **`~/.local/share/agent-brain/brain.db`** (XDG; config ở `~/.config/agent-brain/`). Một DB chung cho mọi dự án để `stats` gộp được. Có biến môi trường ghi đè đường dẫn (cho test). Tên biến cụ thể: Claude quyết.
- **D-07:** **Task = nhánh `task/<slug>`.** `run lead` tạo task (dòng `tasks` + nhánh `task/<slug>` từ HEAD, giữ luật Phase 2 D-14: working tree bẩn thì dừng). `run dev`/`run qa` gắn vào task có nhánh đang checkout trong dự án đó (không ở nhánh task → báo lỗi, không đoán). Mỗi lần chạy một vai trò = một dòng `runs` thuộc task. Commit base ghi trong `.plan.md` (Phase 2 D-14) dùng làm gốc diff.
- **D-08:** `task_id` là **số nguyên ngắn** (rowid SQLite). `run` in ra ngay khi bắt đầu, ví dụ `task 42, run 7`. `agent-brain kill <task_id>` gửi tín hiệu tới process group của run đang chạy của task đó (PGID lưu trong `runs.pid` hoặc cột tương đương).

### Hiển thị khi chạy & kill
- **D-09:** Terminal mặc định: **mỗi sự kiện stream-json một dòng gọn** (giờ, loại, tóm tắt — ví dụ `12:03:41 tool Read src/main.rs`, `assistant: <~100 ký tự đầu>`), cuối run in **một dòng tổng kết** (số turn, token, thời gian, trạng thái). `--verbose` in đầy đủ text assistant và kết quả tool. Toàn bộ log vẫn vào bảng `events` kèm `raw_json`.
- **D-10:** **Ctrl+C lần 1** khi đang `run`: SIGTERM cả process group → chờ thời gian ân hạn → SIGKILL cả group; run ghi trạng thái `killed` kèm token đã nhận được. **Ctrl+C lần 2** trong lúc chờ: SIGKILL ngay. Đóng terminal (SIGHUP) xử lý như Ctrl+C, không hỏi xác nhận.
- **D-11:** Thời gian ân hạn SIGTERM → SIGKILL **mặc định 3 giây, cấu hình được** (ví dụ `kill_grace_secs` trong file cấu hình) — đủ biên cho P2-02 (kiểm tra sau 5 giây). Có thể chỉnh theo kết quả spike S4.
- **D-12:** **Đối soát khi khởi động:** mỗi lần `agent-brain` chạy, các dòng `runs` đang `running` mà process group không còn tồn tại → đánh dấu `lost` (ended_at = lúc phát hiện). `kill` trên task không còn tiến trình thì báo rõ, **không bao giờ gửi tín hiệu tới PID có thể đã bị tái sử dụng** cho tiến trình khác.

### Fast-path & thống kê
- **D-13 (`status`, cấp 1, 0 token):** in nhánh hiện tại, `git status --short`, `git log --oneline -5`, `git diff --stat` so với commit base của task (nếu đang ở nhánh task), và run gần nhất của task kèm kết quả từ SQLite. Không gọi AI, không ghi dòng `runs`.
- **D-14 (`inspect`, cấp 2):** prompt cố định "review ngắn": tóm tắt thay đổi, rủi ro dễ thấy, có khớp `.plan.md` không (nếu có). Diff so với commit base của task, hoặc HEAD nếu không có task. Model **Sonnet**, `--max-turns 1`. Kết quả chỉ in ra terminal; ghi một dòng `runs` với `role = inspect` (để `stats` đếm token). Diff quá lớn thì cắt và báo rõ đã cắt.
- **D-15 (`inspect` không lưu session — P2-07):** ưu tiên cờ CLI không lưu session nếu có (researcher kiểm chứng tên/hành vi cờ); nếu không có hoặc không hoạt động thì **xóa file session theo `session_id`** trong config dir của profile sau khi run xong. P2-07 phải có test.
- **D-16 (`stats`):** tổng (số run, input/output/cache read/cache write token, `cost_usd`) + bảng chia theo **dự án** và theo **vai trò**; `--profile work|personal` lọc (STAT-01); `--since <khoảng>` (ví dụ `7d`) chọn cửa sổ thời gian. `cost_usd` hiển thị nhãn **"tương đương"** vì gói subscription không tính tiền theo token.

### Claude's Discretion
- Tên crate/module chi tiết (bám cấu trúc spec §4.3), định dạng file cấu hình (TOML/JSON), tên biến môi trường ghi đè đường dẫn DB.
- Schema SQLite chi tiết ngoài các cột spec §4.2 (thêm cột cho PGID, trạng thái `killed`/`lost`, base commit, `role = inspect`), cách migrate schema.
- Cách phân loại sự kiện stream-json và rút gọn mỗi dòng hiển thị (dựa trên fixture thật từ `spikes/s1/raw/` nếu đã có).
- Cách đọc 3 prompt vai trò của Phase 2 (nhúng vào binary bằng `include_str!` hay đọc file) — miễn hành vi giống Phase 2 (CLI-03).
- Mã thoát (exit code) cho từng lỗi, miễn Fallback Ask giữ "thoát mã khác 0" như Phase 2 D-09.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 3 source spec
- `files/03-phase2-loi-rust.md` — Mục tiêu, phạm vi, kill sạch cây tiến trình (§3.2), bắt log (§3.3), fast-path 3 cấp (§3.4), lệnh CLI (§4.1), bảng SQLite bản đầu (§4.2), cấu trúc module (§4.3), test case P2-01..P2-09 (§6), rủi ro (§7). §8 ("giao qua giao thức Phase 1") **bị D-01 thay thế**.
- `files/00-ROADMAP.md` — Nguyên tắc xuyên suốt (§1), KPI (§5), câu hỏi mở chính sách tài khoản (§7).

### Planning
- `.planning/ROADMAP.md` §Phase 3 — Goal và 5 success criteria.
- `.planning/REQUIREMENTS.md` — CLI-01..03, PROF-02, PROC-01..02, LOG-01..02, FAST-01..02, STAT-01, DIST-01; FAST-03 (v2, ngoài phạm vi).
- `.planning/PROJECT.md` §Constraints — Tech stack, Distribution, Process safety, Log format tolerance, Protocol, Secrets.

### Prior phase decisions (Phase 3 phải giữ hành vi)
- `.planning/phases/02-protocol-mvp-scripts/02-CONTEXT.md` — Schema `projects_map.json` (D-04..D-07), định tuyến + Fallback Ask (D-08, D-09), chế độ chạy & giới hạn tool từng vai trò (D-10..D-13), nhánh/file giao thức (D-14..D-16), nhật ký (D-17).
- `.planning/phases/01-verify-assumptions/01-CONTEXT.md` — D-06 ánh xạ profile (đã được thay bởi SPK-10 trong `spikes/spike-results.md`: work = `~/.claude`, personal = `~/.claude-orca`), D-08 Linux, D-12 cách kill đã kiểm chứng (S4), D-03 output thô làm fixture.
- Code/prompt/fixture Phase 2 trong `brain/` (tạo ở Phase 2, **chưa tồn tại** khi viết file này) — nguồn hành vi để port.
- `spikes/spike-results.md` và `spikes/s1/raw/`, `spikes/s4/`, `spikes/s5/`, `spikes/s8/` (tạo ở Phase 1, **chưa tồn tại** — Phase 1 chưa chạy) — định dạng stream-json, cách kill, tách profile, `--max-turns`. Planner/researcher phải đọc nếu đã có; nếu chưa, đánh dấu các giả định về cờ CLI là chưa kiểm chứng.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Chưa có code Rust (greenfield). Khi Phase 2 xong sẽ có: `brain/` (Python: chuẩn hóa tên, quét, định tuyến, sinh allowlist tool, đếm turn từ stream-json), 3 file prompt vai trò, mẫu `.plan.md`, bộ test P1-xx — đây là "bản đặc tả chạy được" để port.
- Phase 1 sẽ có `spikes/s1/raw/*.jsonl` (stream-json thật) — fixture cho `stream.rs`.

### Established Patterns
- Hợp đồng từ Phase 2: schema `projects_map.json`, luật chuẩn hóa tên (bỏ dấu, `đ→d`, chữ thường, coi khoảng trắng/`-`/`_` như nhau), Fallback Ask in-và-thoát mã khác 0, profile trống thì từ chối chạy.

### Integration Points
- `~/.config/agent-brain/projects_map.json` (đọc/ghi, chung với Python `brain` trong giai đoạn chuyển tiếp).
- `~/.local/share/agent-brain/brain.db` (mới).
- `claude` CLI headless qua `CLAUDE_CONFIG_DIR` theo profile; các dự án thật (nhánh `task/<slug>`, `.plan.md`, `.qa-report.md`).
- Phase 5 (dashboard) sẽ gắn web server lên lõi này — runner/stream/db nên tách khỏi phần in terminal.

</code_context>

<specifics>
## Specific Ideas

- Người dùng không đọc được Rust: mỗi module cần test theo hành vi và nghiệm thu bằng bảng P2-01..P2-09 (spec §7).
- Dòng tổng kết cuối run nên giúp đo KPI "số turn trước khi sửa code" và token/task.

</specifics>

<deferred>
## Deferred Ideas

- `agent-brain status` không truyền dự án → tổng quan một dòng cho mọi dự án (nhánh, bẩn/sạch, task mở) — tính năng mới, cân nhắc ở Phase 5 (dashboard) hoặc sau.
- Build aarch64 / macOS / Windows (Job Object) — ngoài phạm vi; xem Phase 1 D-08.
- Warm reuse `--resume` (FAST-03) — vẫn ở v2, quyết định ở `/gsd-discuss-phase 4`.

</deferred>

---

*Phase: 03-rust-cli-core*
*Context gathered: 2026-10-14*
