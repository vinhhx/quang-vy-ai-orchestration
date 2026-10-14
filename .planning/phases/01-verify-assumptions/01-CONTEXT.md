# Phase 1: Verify Assumptions - Context

**Gathered:** 2026-10-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Chạy thật 8 spike S1–S8 (Claude Code CLI, prompt cache, kill tiến trình, tách profile, DeepSeek, Antigravity, max-turns), ghi kết quả có bằng chứng vào `spikes/spike-results.md`, và quyết định profile Work/Personal (SPK-01..SPK-10). Không viết code Rust, không xây giao thức Lead/Dev/QA — chỉ thí nghiệm và tài liệu hóa để các phase sau không thiết kế trên giả định sai.

</domain>

<decisions>
## Implementation Decisions

### Cách chạy & lưu bằng chứng
- **D-01:** Agent thực thi các lệnh spike và thu output; người dùng là người kết luận. Đây là điều chỉnh so với tài liệu gốc (§6 "tự chạy tay") — agent được chạy lệnh, nhưng không được tự chốt kết quả.
- **D-02:** Agent ghi vào `spike-results.md` cả Ngày thử, Lệnh, Quan sát (số liệu khách quan) VÀ một kết quả **đề xuất** (Đạt / Không đạt / Một phần) cùng ảnh hưởng tới thiết kế, mỗi mục đánh dấu rõ `(chờ duyệt)`. Người dùng duyệt/sửa rồi bỏ nhãn. Plan phải có checkpoint duyệt của người dùng trước khi phase được coi là xong.
- **D-03:** Output thô (ví dụ `s1.jsonl`, output `--output-format json`, log `ps`) được **commit vào repo** — Phase 3 dùng làm fixture test cho parser stream-json. Trước khi commit phải lọc dữ liệu nhạy cảm (API key, token, email; cân nhắc rút gọn đường dẫn home).
- **D-04:** Vị trí: `spikes/spike-results.md` và `spikes/s1/` … `spikes/s8/` ở **gốc repo** (không đặt trong `.planning/` để không bị archive khi dọn milestone). Mỗi thư mục spike chứa lệnh đã chạy (script `.sh` tái chạy được) và output thô.

### Tài khoản & môi trường thử
- **D-05:** S1–S4 và S8 chạy bằng tài khoản Claude hiện tại (`~/.claude`, gói subscription).
- **D-06:** Profile cho S5: `~/.claude` = **work**; clone `~/.claude-orca` sang `~/.claude-personal` = **personal**. Thay cho `~/.claude-max` trong tài liệu gốc. Không sửa `~/.claude-orca` gốc. Nếu cần đăng nhập lại thì đó là bước tương tác của người dùng.
- **D-07:** S6 kiểm chứng DeepSeek qua **harness Docker hiện có** — container `dsh-deepseek-harness` (image `dsh-base:latest`, đang chạy) — thay vì Claude Code trên host + `ANTHROPIC_BASE_URL`. Cần trả lời: gọi headless từ host được không (vd. `docker exec`), output có cùng định dạng stream-json không, tool call sửa file trong project được mount có hoạt động không, kill từ ngoài có sạch không. Key DeepSeek nằm trong harness; không được ghi key ra file/log commit.
- **D-08:** Máy chuẩn là **máy Linux này**. `agent-brain` nhắm Linux trước (process group / SIGTERM); macOS/Windows ngoài phạm vi phase này. — **Reversibility:** costly — chuyển sang hỗ trợ Windows sau này cần cơ chế kill khác (Job Object) ở Phase 3.

### Ngưỡng Đạt cho spike mơ hồ
- **D-09 (S3 — prompt cache):** KHÔNG dùng `cache_read` tổng làm tín hiệu — system prompt và định nghĩa tool của Claude Code giống nhau giữa các session nên luôn được cache (nhất là khi có session khác cùng tài khoản đang chạy), `cache_read` sẽ không về 0. Đo **phần riêng của dự án** (nội dung `.gsd`/ngữ cảnh dự án): cache ấm → phần này nằm trong `cache_read_input_tokens`; cache hết hạn → chuyển sang `cache_creation_input_tokens`.
- **D-10 (S3 — TTL):** KHÔNG giả định TTL 5 phút. Đo ở 3 mốc: ~1 phút, ~10 phút, ~65 phút để xác định TTL thật trên gói subscription (5 phút hay 1 giờ). Kết quả này quyết định thiết kế "golden session" ở Phase 4 (OPT/SESS). Ảnh hưởng tới quota subscription: ghi nếu quan sát được, không chặn phase.
- **D-11 (S7 — Antigravity):** Tra tài liệu chính thức; nếu máy đã cài Antigravity thì chạy thử lệnh headless/JSON. Không cài mới chỉ để làm spike. Nếu chỉ dựa trên docs, ghi rõ điều đó.
- **D-12 (S4 — kill):** Hai bước: (1) kill PID `claude` khi agent đang chạy `sleep 300` qua bash, ghi tiến trình sót; (2) chạy trong process group riêng (`setsid`) và kill cả nhóm (`kill -- -PGID`), kiểm tra sạch. Phase 3 có sẵn phương án đã kiểm chứng.
- **D-13 (Cổng đóng phase):** Phase đóng khi cả 8 spike có kết quả đã được người dùng duyệt, và mọi spike "Một phần"/"Không đạt" ghi rõ phương án thay thế + phase bị ảnh hưởng. Riêng **S1 (stream-json) và S4 (kill)** — nền của Phase 3 — bắt buộc phải có cách làm rõ ràng, đã thử.

### Claude's Discretion
- Cấu trúc chi tiết bên trong `spikes/sN/` (tên file, script) và cách lọc dữ liệu nhạy cảm.
- Prompt/task cụ thể dùng cho S3 để có phần "riêng dự án" đủ lớn, đo được.
- Cách gọi harness Docker cho S6 (sau khi tìm hiểu cấu hình container).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 1 source spec
- `files/01-phase0-kiem-chung.md` — Danh sách spike S1–S8, lệnh thử, tiêu chí nghiệm thu (§4), mẫu `spike-results.md` (§5). Lưu ý: D-01..D-13 ở trên ghi đè §6 (cách giao cho agent), S5 (`~/.claude-max` → `~/.claude-personal`), S6 (dùng harness Docker), S3 (mốc thời gian và cách đo).
- `files/00-ROADMAP.md` — Nguyên tắc xuyên suốt (§1), câu hỏi mở về chính sách tài khoản công ty (§7).

### Planning
- `.planning/ROADMAP.md` §Phase 1 — Goal và 4 success criteria.
- `.planning/REQUIREMENTS.md` — SPK-01..SPK-10.
- `.planning/STATE.md` — Blocker về chính sách công ty (SPK-10) và S7 → ENG-06.

### Downstream consumers of spike results
- `files/03-phase2-loi-rust.md` — Phase 3 (Rust core) dùng kết quả S1 (parser stream-json), S4 (kill), S8 (fast-path).
- `files/04-phase3-toi-uu-session.md` — Phase 4 dùng kết quả S2, S3 (cache/TTL, golden session).
- `files/06-phase5-da-doi-hinh-media.md` — Phase 6 dùng S6 (DeepSeek), S7 (Antigravity).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Chưa có code dự án (greenfield). Repo chỉ có `files/` (tài liệu gốc), `.planning/` và bản cài GSD trong `.claude/`.
- Container `dsh-deepseek-harness` (image `dsh-base:latest`) đang chạy trên máy — điểm vào cho S6.
- Thư mục cấu hình Claude hiện có: `~/.claude` (work), `~/.claude-orca` (nguồn clone cho personal).

### Established Patterns
- Không có — phase này tạo ra fixture (`spikes/sN/`) mà Phase 3 sẽ dùng.

### Integration Points
- `spikes/` ở gốc repo là đầu vào cho Phase 3 (fixture test parser stream-json, chiến lược kill) và Phase 4/6.

</code_context>

<specifics>
## Specific Ideas

- Người dùng nhấn mạnh: nếu TTL cache trên subscription là 1 giờ thì mốc 10 phút vẫn hit cache và kết luận về "golden session" ở Phase 4 sẽ khác hẳn — vì vậy mốc ~65 phút là bắt buộc.
- DeepSeek trên máy này đang được dùng qua "deepseek harness trong docker", không phải qua `ANTHROPIC_BASE_URL` trên host.

</specifics>

<deferred>
## Deferred Ideas

- Chính sách công ty cho tài khoản Work/Personal (SPK-10) — không thảo luận trong buổi này; vẫn là blocker mở trong STATE.md, cần xác nhận trước Phase 3.
- Kịch bản kill có MCP server con và kill harness Docker từ ngoài ở mức đầy đủ — S4 chỉ làm process group cơ bản; khía cạnh Docker được trả lời một phần trong S6.
- Hỗ trợ macOS/Windows cho `agent-brain` — ngoài phạm vi; Linux trước.

</deferred>

---

*Phase: 01-verify-assumptions*
*Context gathered: 2026-10-14*
