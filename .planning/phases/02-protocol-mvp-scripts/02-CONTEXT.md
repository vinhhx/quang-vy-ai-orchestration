# Phase 2: Protocol MVP (Scripts) - Context

**Gathered:** 2026-10-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Chạy được quy trình Lead → `.plan.md` → Dev → QA trên dự án thật chỉ bằng script và file cấu hình (ROUTE-01..06, ROLE-01..07, PROF-01): định tuyến dự án theo tên tiếng Việt/alias với luật Fallback Ask, quét/đăng ký dự án vào `projects_map.json`, 3 prompt vai trò, lệnh `brain`, giới hạn tool theo vai trò, chọn profile qua `CLAUDE_CONFIG_DIR`, và 5 task thật có nhật ký. Ngoài phạm vi: dashboard, SQLite, tối ưu session, chạy song song nhiều task, Rust (Phase 3).

</domain>

<decisions>
## Implementation Decisions

### Ngôn ngữ, vị trí & lệnh
- **D-01:** Viết bằng **Python** (thư viện chuẩn là chính; bỏ dấu tiếng Việt bằng `unicodedata` + xử lý riêng `đ/Đ → d`). Đây là công cụ tạm: sau Phase 3, binary Rust `agent-brain` thay thế. Thứ có giá trị lâu dài và phải được thiết kế cẩn thận là: 3 file prompt, mẫu `.plan.md`, schema `projects_map.json`, bộ test P1-01..P1-10 — Phase 3 port sang Rust và phải cho kết quả giống (CLI-02, CLI-03). Người dùng đã cân nhắc phương án viết Rust ngay và chọn không làm (giữ nguyên tắc "chứng minh giao thức trước khi đầu tư Rust").
- **D-02:** Code `brain` và 3 file prompt nằm **trong repo này** (ví dụ thư mục `brain/`), cài bằng symlink `brain` vào PATH. `projects_map.json` nằm ở **`~/.config/agent-brain/projects_map.json`** (chứa đường dẫn riêng của máy, không commit; Phase 3 đọc lại cùng file).
- **D-03:** Một lệnh `brain` có subcommand: `brain scan`, `brain add <path>`, `brain lead "<dự án>" "<yêu cầu>"`, `brain dev "<dự án>"`, `brain qa "<dự án>"` — cùng hình dạng CLI `agent-brain` ở Phase 3.

### `projects_map.json` (là "database" dự án ở Phase 2)
- **D-04:** Không có một thư mục gốc duy nhất. Mỗi dự án lưu **`path` tuyệt đối riêng** và **`repo` = URL git remote** đọc từ `git remote get-url origin` (null nếu không có remote). Thay `"root"` trong spec bằng `"scan_roots": [...]` (một hoặc nhiều thư mục). Giữ các trường còn lại của spec §4.1: `id`, `name_vi`, `aliases`, `type` (`code`|`media`), `profile`. — **Reversibility:** costly — schema được Phase 3 (`projects.rs`, SQLite) và Phase 6 (`type: media`) đọc lại; đổi tên trường cần migrate file map.
- **D-05:** Dự án vào map bằng hai đường: `brain scan` (quét `scan_roots`) và `brain add <path>` (dự án nằm ngoài roots, kể cả thư mục media không dùng git). Cả hai **không bao giờ ghi đè** `name_vi`/`aliases`/`profile` đã sửa tay (ROUTE-05) và **cảnh báo alias trùng** giữa các dự án (ROUTE-06).
- **D-06:** `brain scan` chỉ tính **thư mục con trực tiếp (độ sâu 1) của mỗi scan_root có `.git`** → `type: code`. Dự án trong map mà thư mục không còn tồn tại thì đánh dấu `missing`, không tự xóa khỏi map.
- **D-07:** Dự án mới: `id` = tên thư mục, `name_vi` mặc định = tên thư mục, `aliases` rỗng, **`profile` trống — bắt buộc đặt tay**; `brain` từ chối chạy agent cho dự án chưa có profile (tránh vô tình dùng tài khoản công ty cho dự án cá nhân khi SPK-10 chưa chốt).

### Định tuyến & Fallback Ask
- **D-08:** So khớp **chính xác sau chuẩn hóa**: bỏ dấu (cả `đ→d`), chữ thường, coi khoảng trắng/`-`/`_` là như nhau, gộp khoảng trắng; so với `id`, `name_vi` và từng alias. Không khớp chuỗi con, không fuzzy. Ví dụ "web bán hàng" = `web-ban-hang`.
- **D-09:** Fallback Ask: khớp 0 hoặc >1 dự án → **in danh sách** (các dự án khớp, hoặc toàn bộ nếu 0 khớp) và **thoát với mã lỗi khác 0**, không chạy agent, không hỏi tương tác. Người dùng gọi lại bằng `id` chính xác. (Dễ test P1-03/P1-04; Phase 3/5 gọi từ chương trình khác được.)

### Chế độ chạy & giới hạn tool
- **D-10:** Cả 3 vai trò chạy **headless `claude -p`** với `--output-format stream-json` (kèm cờ cần thiết, ví dụ `--verbose`), log stream-json lưu lại cho mỗi lần chạy — dùng để đếm turn cho nhật ký và làm dữ liệu cho Phase 3. Plan chưa ưng thì người dùng sửa tay `.plan.md` hoặc chạy lại `brain lead`.
- **D-11 (Lead):** Được Read/Grep/Glob; chỉ được ghi **`.plan.md`** (permission rule giới hạn Write/Edit vào đúng file này); Bash chỉ các lệnh git đọc (`git log`, `git show`, `git diff`, `git status`). Sau khi Lead xong, script kiểm tra `git status` — có file khác bị sửa thì báo lỗi (ROLE-01).
- **D-12 (Dev):** Script **sinh allowlist từ cột File trong bảng "Thay đổi" của `.plan.md`**: Dev chỉ được Read/Edit đúng các file đó + `.plan.md` (để ghi "Vướng mắc"); cấm Grep/Glob/WebFetch/WebSearch; Bash chỉ `git add`/`git commit`. Dev **không** được chạy test — script tự chạy phần "Cách kiểm tra" sau khi Dev xong (hoặc in ra cho người dùng chạy). Đây là cách thực thi ROLE-06 bằng giới hạn công cụ, không chỉ bằng prompt.
- **D-13 (Model):** Lead dùng **Opus**, Dev và QA dùng **Sonnet**; ghi trong file cấu hình để đổi được.

### Nhánh, file giao thức & nhật ký
- **D-14:** `brain lead` tạo nhánh **`task/<slug>`** (slug không dấu từ yêu cầu, hoặc cờ `--name`) từ **HEAD hiện tại**. Working tree có thay đổi chưa commit → dừng và báo, không tự stash. Nhánh base/commit base được ghi vào `.plan.md` để QA biết diff từ đâu.
- **D-15:** `.plan.md` và `.qa-report.md` ở **gốc dự án** và **được commit trên nhánh task**: script commit `.plan.md` ngay sau Lead (message `"<task>: plan"`), Dev commit code lên trên (message `"<task>: <mô tả ngắn>"`, ROLE-02), QA diff từ commit plan đến HEAD nên không lẫn `.plan.md`. Giữ lịch sử cho Phase 4 (`.handover.md`).
- **D-16:** QA kết luận "Cần sửa" → script **chỉ báo**, người dùng tự quyết (sửa `.plan.md` hoặc chạy lại `brain lead`, rồi `brain dev` lần nữa). Dev không đọc `.qa-report.md` — giữ ranh giới "Dev chỉ đọc plan". Mỗi lần làm lại được ghi vào nhật ký.
- **D-17:** Nhật ký task (ROLE-07): **script tự điền số liệu** từ log stream-json — số turn của Dev trước lần Edit đầu tiên, các file Dev đọc ngoài plan, token; **người dùng điền** "QA phát hiện gì" và "phải làm lại không, vì sao". Lưu trong repo này (ví dụ `protocol-log/`), theo mẫu §7 của spec.

### Claude's Discretion
- Cấu trúc module Python, tên file prompt, cách cài symlink.
- Giới hạn tool chi tiết của **QA** (không bàn trực tiếp): theo bảng vai trò của spec — không sửa code, chỉ ghi `.qa-report.md`, Bash chỉ git đọc; được Read file khác ở chế độ chỉ đọc để đánh giá "ảnh hưởng tới chỗ khác" nếu cần.
- Cách Lead xử lý khi dự án không có `.gsd` (định dạng `.gsd` chưa có tài liệu — xem STATE.md): đọc `.gsd` nếu có, nếu không thì dùng CLAUDE.md/README.
- Dev tự commit qua Bash hay script commit thay sau khi Dev xong — miễn kết quả đúng ROLE-02.
- Ví dụ "plan tốt" đưa vào prompt Lead (mitigation rủi ro "Plan quá chung chung", spec §8).
- Cách test P1-01..P1-10 (tự động cho phần định tuyến/quét; phần agent thì kịch bản thử tay có hướng dẫn).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 2 source spec
- `files/02-phase1-giao-thuc-mvp.md` — Schema `projects_map.json` (§4.1, bị D-04 sửa: `root` → `scan_roots`, `repo` = URL remote), mẫu `.plan.md` (§4.2), bản nháp 3 prompt (§4.3), lệnh `brain` (§4.4), test case P1-01..P1-10 (§6), mẫu nhật ký (§7), rủi ro (§8).
- `files/00-ROADMAP.md` — Nguyên tắc xuyên suốt (§1), cổng Phase 1 nguồn (§2), câu hỏi mở chính sách tài khoản (§7).

### Planning
- `.planning/ROADMAP.md` §Phase 2 — Goal và 5 success criteria.
- `.planning/REQUIREMENTS.md` — ROUTE-01..06, ROLE-01..07, PROF-01; CLI-02/CLI-03 (Phase 3 phải giữ hành vi script).
- `.planning/PROJECT.md` §Constraints — "Protocol" (chỉ trao đổi qua file, cấm bằng giới hạn tool), "Secrets".

### Prior phase decisions
- `.planning/phases/01-verify-assumptions/01-CONTEXT.md` — D-06: profile work = `~/.claude`, personal = `~/.claude-personal` (thay `~/.claude-max`); D-08: Linux. Phase 2 dùng ánh xạ này cho `CLAUDE_CONFIG_DIR` (PROF-01).
- `spikes/spike-results.md` (tạo ở Phase 1, **chưa tồn tại** — Phase 1 chưa chạy) — S1 (định dạng stream-json, dùng để đếm turn/D-17), S5 (tách profile), S8 (`--max-turns`). Planner/researcher phải đọc nếu đã có; nếu chưa, đánh dấu các giả định về cờ CLI là chưa kiểm chứng.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Chưa có code dự án (greenfield). Repo chỉ có `files/` (tài liệu gốc), `.planning/` và bản cài GSD trong `.claude/`.
- Phase 1 sẽ tạo `spikes/s1/raw/` (output stream-json thật) — dùng làm fixture để viết/kiểm parser đếm turn của D-17.

### Established Patterns
- Chưa có. Phase 2 đặt ra các "hợp đồng" (schema map, mẫu plan, prompt) mà Phase 3 port nguyên sang Rust.

### Integration Points
- `~/.config/agent-brain/projects_map.json` — đọc lại bởi Phase 3 (`projects.rs`, PROF-02).
- `CLAUDE_CONFIG_DIR` theo `profile` — ánh xạ từ Phase 1 D-06.
- Các dự án thật mà `brain` chạy trên: nhánh `task/<slug>`, `.plan.md`, `.qa-report.md` ở gốc dự án.

</code_context>

<specifics>
## Specific Ideas

- Người dùng muốn mỗi dự án được quản lý như một bản ghi "database" có `path` và repository riêng, thay vì ngầm định từ một thư mục gốc.
- Người dùng hỏi "tech stack là Rust thì nên dùng gì" — đã thống nhất: Python cho Phase 2 vì sẽ bỏ đi; giá trị chuyển sang Rust là hợp đồng + test case, không phải code script.

</specifics>

<deferred>
## Deferred Ideas

- Hỏi chọn số tương tác khi Fallback Ask (có TTY) — đã chọn in-và-thoát; có thể xem lại ở Phase 3/5 (dashboard có thể cho chọn).
- `brain dev --fix` đọc `.qa-report.md` để tự sửa vòng lặp QA → Dev — bị từ chối vì lệch giao thức; có thể xem lại khi có số liệu làm lại từ nhật ký.
- Quét đệ quy nhiều cấp thư mục — chưa cần; dùng `brain add` cho dự án lồng sâu.
- Chính sách công ty cho Work/Personal (SPK-10) — vẫn là blocker mở từ Phase 1; Phase 2 né bằng D-07 (profile bắt buộc đặt tay).

</deferred>

---

*Phase: 02-protocol-mvp-scripts*
*Context gathered: 2026-10-14*
