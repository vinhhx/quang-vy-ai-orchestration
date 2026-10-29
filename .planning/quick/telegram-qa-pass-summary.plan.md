# Plan: khi QA đạt, Telegram gửi file tóm tắt (yêu cầu, cách làm, kết quả)

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Yêu cầu của người dùng (2026-10-29)
Khi QA duyệt đạt, bot Telegram gửi một FILE tóm tắt để người dùng biết: công việc yêu cầu là gì, đã thực hiện thế nào, kết quả ra sao. Hiện tin "QA đạt · task N" chỉ có tiêu đề, tên dự án và một dòng tổng số run/phút/chi phí (`telegram/render.rs` ~dòng 310), không có nội dung.

## Cơ chế sẵn có (đã đọc, dùng lại, không viết lại)
- Tin Gate đã gửi kèm file plan: `render::Rendered.document: Option<(file_name, content)>`, gửi trong `telegram/outbox.rs` ~dòng 296-335 bằng `sendDocument` (`Body::Upload`, `DOCUMENT_MAX_SECS`, `CAPTION_MAX`), có retry và `doc_done` để không gửi lại file đã giao.
- `render::message_for` chỉ đặt `document` cho Gate. `bot.rs` (~dòng 1043, 1131) cũng gọi `message_for` để sửa tin; KHÔNG được làm việc nặng (git, đọc file) trong `message_for`.
- `detect::current_event` trả `EventKind::QaPass` với key `qa:{run id}` khi `tasks.status == "qa_pass"`. Giữ nguyên.
- `TaskState` (`dashboard/api_tasks.rs`): `task` (title, slug, base_commit, branch), `root`, `plan_text`, `runs` (role, engine, status, outcome, stage, round, cost_usd, started_at, ended_at), `workflow`, `team`.
- Hàm git có sẵn trong `gitops.rs`: `git(root,&[..])`, `commit_subjects`, `diff_names`.

## Nội dung file tóm tắt
Tên file `<slug>.summary.md` (slug hợp lệ theo `slug_ok`, nếu không thì `summary.md`). Markdown tiếng Việt, đúng thứ tự:
1. `# Tóm tắt task <id>: <title>` + dòng dự án, nhánh, quy trình, đội hình.
2. `## Yêu cầu` — trường `request` trong `<!-- brain-meta: ... -->` của `.plan.md` (dùng `planfile::read_meta`); không có thì dùng `task.title`.
3. `## Cách thực hiện` — phần `## Mục tiêu` của `.plan.md` (dùng `planfile::parse_plan`, lấy nguyên văn mục tiêu, tối đa 3000 ký tự) và danh sách file trong bảng Thay đổi (`planfile::plan_files`, tối đa 50 file); rồi bảng các run: vai, engine, trạng thái/outcome, số phút.
4. `## Kết quả` — (a) commit của nhánh từ `task.base_commit` tới `HEAD`: `<hash7> <subject>`, tối đa 50 dòng; (b) `git diff --stat <base>..HEAD` rút gọn: tổng số file, +dòng, −dòng và tối đa 100 file kèm số dòng đổi; KHÔNG chèn nội dung diff; (c) kết luận QA lấy từ `.qa-report.md` trong `root` (nếu có): khối front matter (`specs`, `failing`, `not_automatable`) và các mục của báo cáo, cắt tổng cộng ≤ 8000 ký tự, kết thúc bằng `… (đã cắt)` khi cắt.
5. `## Tổng` — dùng lại `render::summary_line`.
Thiếu dữ kiện nào (không có `.plan.md`, không có base_commit, git lỗi, không có `.qa-report.md`) thì ghi đúng một dòng `(không có …)` ở mục đó, không làm hỏng cả file và không panic.
Không đọc `.env`, key hay file ngoài `.plan.md`, `.qa-report.md` và đầu ra git ở trên. Chạy qua `secrets::redact_str` (nếu có hàm dùng chung cho văn bản) trước khi gửi.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/telegram/summary.rs` (file mới) | — | Hai phần: (a) `pub struct SummaryInput { ... }` thuần dữ liệu (title, slug, id, project, branch, workflow, team, request, objective, plan_files, runs: Vec<(role, engine, status_word, minutes)>, commits: Vec<String>, stat: Option<DiffStat>, qa_report: Option<String>, total_line) và `pub fn build(input:&SummaryInput)->String` thuần, dễ test; (b) `pub fn document(s:&TaskState)->(String,String)` gom dữ kiện (git qua `gitops`, đọc `.plan.md` từ `s.plan_text`, đọc `.qa-report.md` từ `s.root`) rồi gọi `build`. Giới hạn kích thước như mục trên, cắt an toàn UTF-8. | Sinh file tóm tắt xác định, không dùng LLM |
| 2 | `agent-brain/src/telegram/mod.rs` | khai báo module | Thêm `pub mod summary;` đúng chỗ các module khác được khai báo. | Đăng ký module |
| 3 | `agent-brain/src/telegram/outbox.rs` | ~dòng 304-312 (sau `let r = render::message_for(ev, &s);`) | Làm `let mut r`; nếu `ev.kind == EventKind::QaPass && r.document.is_none()` thì `r.document = Some(summary::document(&s));`. Caption theo loại sự kiện: Gate giữ `Plan task {id}: {title}`; QaPass dùng `Tóm tắt task {id}: {title}`. Không đổi logic retry, `doc_done`, `doc_message`. | Gửi file trước tin chữ như Gate |
| 4 | `agent-brain/src/telegram/render.rs` | `text_for`, nhánh `EventKind::QaPass` (~dòng 310-315) | Thêm một dòng sau `summary_line`: `Tóm tắt (yêu cầu, cách làm, kết quả) ở file đính kèm ngay trên.` và, nếu đọc được, một dòng kết luận QA ngắn từ front matter của `.qa-report.md` (ví dụ `QA: <specs> spec, <failing> lỗi, <not_automatable> không tự động hóa được`) — chỉ khi hàm đọc đã có sẵn trong `TaskState` hoặc dùng được mà không thêm IO nặng; nếu không thì bỏ dòng này. KHÔNG sửa `message_for` để làm IO. Cập nhật doc-comment của `Rendered.document` (không còn "gate only"). | Tin chữ trỏ tới file |
| 5 | `agent-brain/src/telegram/summary.rs` (module `tests`) | cuối file | Test: (a) `build` có đủ 5 mục theo thứ tự với dữ liệu đầy đủ; (b) thiếu từng dữ kiện (không request, không plan, không commit, không qa_report) vẫn ra file hợp lệ với dòng `(không có …)`; (c) cắt đúng giới hạn (mục tiêu 3000, QA 8000, 50 commit, 100 file) và an toàn UTF-8 (chuỗi tiếng Việt cắt giữa ký tự nhiều byte); (d) tên file `<slug>.summary.md` và `summary.md` khi slug lạ. | Chốt hành vi |
| 6 | `agent-brain/tests/telegram_notify.rs` (hoặc `telegram_actions.rs`, chọn file có sẵn test QA đạt ở ~dòng 475 và fake Telegram ghi lại `sendDocument`) | cuối file | Test tích hợp: task đạt QA qua autopilot/CLI giả → fake Telegram nhận `sendDocument` có `filename` `<slug>.summary.md`, nội dung chứa `## Yêu cầu`, `## Cách thực hiện`, `## Kết quả`, rồi mới tới `sendMessage` chứa `QA đạt · task`. Test Gate cũ giữ nguyên và pass. Nếu fake Telegram chưa ghi được `sendDocument`, theo mẫu test Gate đang dùng. | Kiểm cả luồng |
| 7 | `agent-brain/README.md` | mục Telegram: thông báo (tìm chỗ mô tả "QA đạt" / file plan của cổng) | Ghi: khi QA đạt, bot gửi file `<slug>.summary.md` (yêu cầu, cách làm, kết quả; không có nội dung diff) rồi tin chữ. Nêu giới hạn kích thước và rằng thiếu dữ kiện thì ghi `(không có …)`. | Tài liệu |

## Không được đụng tới
- `detect.rs` (điều kiện và key sự kiện), `callback.rs`, `bot.rs`, `transport.rs`, cách retry/`doc_done` trong `outbox.rs`.
- Tin Gate, Stop, Quota, RunFailed: nội dung và file đính kèm giữ nguyên.
- `~/.config/agent-brain/*`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- Không đọc, in hay ghi nội dung `.claude.json`, `telegram.json` hay bất kỳ token nào; test dùng token giả.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --lib telegram 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked --test telegram_notify --test telegram_actions --test telegram_outage --test telegram_quota 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked 2>&1 | grep -E "FAILED|failed" | head          # toàn bộ: không có lỗi
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/telegram/(summary|outbox|render)\.rs" | head
```
Acceptance:
1. Task đạt QA: bot gửi `sendDocument` `<slug>.summary.md` rồi `sendMessage` "QA đạt · task N" có dòng trỏ tới file.
2. File có 5 mục theo thứ tự, không chứa nội dung diff, không vượt giới hạn.
3. Thiếu `.plan.md`, base_commit, git lỗi hay `.qa-report.md` không làm hỏng việc gửi.
4. Gửi lại sau khi mạng lỗi không gửi file hai lần (hành vi `doc_done` giữ nguyên, có test cũ phủ).
5. Mọi test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): Telegram sends a task summary file when QA passes` (code và test), `docs: Telegram QA summary file` (README). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Nếu `cargo fmt` đổi file ngoài bảng thì hoàn tác file đó trước khi commit.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify, và nói rõ phần nào chưa kiểm (ví dụ chưa gửi thật qua Telegram).
