# Phase 7: Journal Vault (Obsidian) - Context

**Gathered:** 2026-11-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Xuất nhật ký công việc hằng ngày từ `brain.db` sang vault Obsidian riêng tư `git@github.com:vinhhx/agent-brain-journal.git`. Một chiều, chỉ đọc trên điện thoại (Obsidian + obsidian-git, pull). Kèm digest Telegram cuối ngày và lệnh `/tim` (SQLite FTS5). Mọi hành động vẫn đi qua Telegram → agent-brain; vault không ghi ngược.

Ngoài phạm vi: ghi ngược từ vault, file tuần, nhật ký theo task, xem diff/prompt/log trong vault.

</domain>

<decisions>
## Implementation Decisions

### Đã thống nhất trước (2026-11-10, memory `journal-vault-idea`)
- **D-01:** Repo vault riêng tư, chỉ chứa dữ liệu; code nằm trong agent-brain. brain.db là nguồn sự thật, vault dựng lại được bất cứ lúc nào.
- **D-02:** Layout: `Today.md` (embed file hôm nay `![[...]]`, link hôm qua và tháng), `Nhật ký/YYYY/MM/YYYY-MM-DD.md`, tháng `YYYY-MM.md`, năm `YYYY.md`, `Dự án/<project>.md`. Chỉ ngày/tháng/năm, không có file tuần. Quy mô 10–30 task/ngày.
- **D-03:** Không đưa vào vault: diff, prompt, path, env, key, endpoint, account. Chỉ field allowlist, có quét secret trước khi push.

### Nguồn chữ trong nhật ký
- **D-04:** Câu mô tả mỗi task ghép từ **mẫu cố định theo allowlist** (tiêu đề, dự án, trạng thái, vai đã chạy). Không gọi model → không tốn token, không đọc plan/QA/diff nên không thể lộ chúng.
- **D-05:** Tiêu đề/yêu cầu gốc do người dùng gõ được dùng **nguyên văn nhưng cắt gọn (~140 ký tự) và qua bộ quét secret**; chặn path và URL nội bộ.
- **D-06:** **Một dòng mỗi task mỗi ngày**: trạng thái cuối trong ngày kèm số vòng. Task nhiều ngày xuất hiện lại ở ngày có thay đổi/hoàn tất.

### Phân loại 3 nhóm
- **D-07:** Nhóm (Công việc / Video / Hỗ trợ bộ phận khác) theo **cấu hình dự án** (`journal_group`, khai một lần). Không đoán theo từ khóa, không đánh nhãn theo task.
- **D-08:** Dự án chưa khai nhóm → **Công việc**.

### Chạy, commit, push
- **D-09:** Exporter chạy **trong `agent-brain serve`** (vòng nền ~30 phút và cuối ngày) và có lệnh tay `agent-brain journal export` để chạy/dựng lại thủ công. Dùng cơ chế pump hiện có của serve.
- **D-10:** Push lỗi (mạng, SSH, xung đột): **commit cục bộ, thử lại lần sau, báo Telegram sau N lần liên tiếp**. Nếu pull xung đột (có người sửa vault) thì dừng và báo, **không ghi đè**.
- **D-11:** Vault clone tại `~/.local/share/agent-brain/journal`, tác giả commit `agent-brain journal` (danh tính riêng).
- **D-12:** Ngày tính theo **giờ địa phương máy (Asia/Ho_Chi_Minh)**, đổi ngày lúc 00:00; brain.db lưu epoch ms nên quy đổi khi xuất.

### Digest Telegram và /tim
- **D-13:** Digest cuối ngày, **18:00 và cấu hình được**: số task theo nhóm, mục "cần anh xử lý", link file ngày.
- **D-14:** Link là **github.com tới file ngày** (repo private, mở khi đã đăng nhập GitHub), không dùng `obsidian://`.
- **D-15:** `/tim` (SQLite FTS5 trong brain.db) chỉ index **nội dung đã allowlist của nhật ký** (tiêu đề, dự án, nhóm, ngày, trạng thái); trả về ngày + link, không lộ thêm so với vault.
- **D-16:** Mục "cần anh xử lý" ở đầu file ngày gồm: **plan chờ duyệt (cổng)**; **hết quota / môi trường sập**; **QA "Cần sửa" hoặc vướng mắc chưa xử lý**; **run lỗi / timeout / protocol_violation**.

### Claude's Discretion
- Tên bảng/migration (lược đồ DB tiếp theo), cấu trúc module (`journal`), mẫu câu cụ thể, giá trị N cho báo lỗi push, định dạng frontmatter của file ngày, định dạng nhãn trạng thái tiếng Việt, tên trường cấu hình digest.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Thiết kế đã thống nhất
- `/home/pc0741/.claude-max/projects/-mnt-data-IDE-QuangVy-Orchestration/memory/journal-vault-idea.md` — thiết kế gốc (vault, layout, allowlist, digest, `/tim`)
- `.planning/ROADMAP.md` §Phase 7 — mục tiêu phase

### Mã hiện có liên quan
- `agent-brain/src/db.rs` — lược đồ và migration (tasks, runs, intake, proposals, quota_stops, plan_approvals…); thêm migration mới ở đây
- `agent-brain/src/telegram/` (`bot.rs`, `outbox.rs`, `render.rs`, `callback.rs`) — gửi digest, thêm lệnh `/tim`
- `agent-brain/src/orchestrator/scheduler.rs`, `orchestrator/mod.rs` — vòng nền trong serve
- `agent-brain/src/secrets.rs`, `spikes/lib/scan-secrets.sh` — bộ quét secret
- `agent-brain/src/projects.rs`, `config.rs` — cấu hình dự án (`journal_group`)
- `CLAUDE.md` — quy trình Claude lập plan, agy thực thi, Claude review

No external specs — quyết định nằm ở trên.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Telegram bot (long polling, outbox, lệnh đã có `/task`, fixbug) → thêm `/tim` và digest.
- Vòng nền của `serve` (pump `PRAGMA data_version`, scheduler tick) → lịch xuất 30 phút/cuối ngày.
- Bộ quét secret → chặn trước khi commit.
- Bảng `tasks`, `runs`, `quota_stops`, `plan_approvals` → nguồn dữ liệu allowlist.

### Established Patterns
- Migration lược đồ tuần tự trong `db.rs`; sao lưu brain.db trước khi nâng.
- Không hard code: mọi số (giờ digest, chu kỳ, N lần thử) nằm trong cấu hình.
- Chỉ cài binary mới khi restart `serve`, không thay khi task đang chạy.

### Integration Points
- Thêm module `journal` (dựng file Markdown từ brain.db, git clone/commit/push, FTS5 index).
- Cấu hình dự án thêm `journal_group`; cấu hình `journal` (repo URL, giờ digest, chu kỳ).

</code_context>

<specifics>
## Specific Ideas

- Nội dung bằng tiếng Việt thường, đọc như "hôm nay đã làm gì".
- Mục "cần anh xử lý" luôn ở đầu file ngày.

</specifics>

<deferred>
## Deferred Ideas

- Ghi ngược/giao việc từ vault (Telegram vẫn là đường duy nhất).
- Tóm tắt bằng model cho cuối ngày (đã chọn mẫu cố định; có thể xem lại sau).
- Tìm trong plan/QA report qua `/tim`.

</deferred>

---

*Phase: 07-journal-vault-obsidian*
*Context gathered: 2026-11-10*
