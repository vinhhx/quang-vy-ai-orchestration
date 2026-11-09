# Plan fixbug-trial: giao việc fix bug cho agent-brain chạy được (2026-11-09)

Mục tiêu: hôm nay người dùng thử giao fix bug cho agent-brain trên bizsurvey (sau đó apec). Mỗi bug Trello thành một task, không trùng với skill `/fix-bugs` cũ, Lead nhìn được ảnh QA dán, người dùng ra lệnh được từ Telegram, và các task code phụ thuộc nhau chạy đúng thứ tự.

Code hiện tại: `main` tại `c4bc990`. Đọc trước:
- `agent-brain/src/fixbug/{mod,pull,init,writeback,config,trello}.rs`
- `agent-brain/src/orchestrator/{mod,triage,scheduler}.rs`
- `agent-brain/src/telegram/{bot,intake,render}.rs`, `agent-brain/src/roles/lead.rs` (`wants_untrusted_prompt`)
- Skill cũ để hiểu dữ liệu cần nhập: `/mnt/data/source_codes/code/php/company/laravel/bizsurvey/.claude/skills/fix-bugs/SKILL.md` mục 2–3 (chỉ đọc, không sửa repo đó).

Chỉ sửa trong:
- `agent-brain/src/**`
- `agent-brain/tests/**`
- `agent-brain/assets/**`
- `agent-brain/README.md` (mục lệnh fixbug và Telegram)

## 0. Trước khi sửa
Ghi lại số liệu nền để so ở cuối:
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"
```

## Phase A — một bug = một task, không trùng với skill cũ

### A1. Pull không tách bug
`src/fixbug/pull.rs:313` đang gọi `insert_intake(..., true)` nên triage có thể tách một bug thành tới 5 task. Sửa thành `false`. Cập nhật test trong `tests/fixbug_pull.rs` để khẳng định intake của bug có `split = 0`.

### A2. Giữ nguyên nội dung bug qua triage
`orchestrator/mod.rs::tick`: với intake có `source` là `trello` hoặc `sheet`, sau khi triage trả về 1 đề xuất hợp lệ thì:
- `request` của đề xuất = **nguyên văn `intake.request`** (không dùng bản LLM viết lại, để giữ link, comment, khối dữ liệu QA, danh sách ảnh).
- `title` = `"<KEY> <tiêu đề card>"`, lấy từ `fixbug_refs` theo `intake_id` (cắt 120 ký tự).
- Giữ `level`, `workflow`, `team`, `priority` của triage. Nếu triage trả `priority` khác với intake thì dùng priority của intake (label Trello là nguồn đúng).

Viết hàm thuần `apply_fixbug_override(draft, intake, ref) -> ProposalDraft` để test được không cần engine.

### A3. Khối dữ liệu QA + luật không tin cậy cho Lead
- `pull.rs`: bọc phần mô tả, bình luận, đính kèm của card/hàng sheet trong khối:
  ```
  <<<DU_LIEU_QA <mã ngẫu nhiên 8 hex>
  ...
  <<<HET_DU_LIEU_QA <cùng mã>
  ```
  Nếu nội dung QA có chứa chuỗi `<<<HET_DU_LIEU_QA` thì thay `<<<` trong nội dung bằng `‹‹‹` trước khi bọc. Bỏ dòng "⚠ Nội dung dưới đây là DỮ LIỆU…" cũ (luật chuyển sang prompt).
- `roles/lead.rs::wants_untrusted_prompt`: trả `true` khi (điều kiện cũ) **hoặc** `request` chứa `<<<DU_LIEU_QA `. Dùng chung hằng số mở/đóng khối, đặt ở `fixbug/mod.rs`.
- `assets/prompts/lead-untrusted.md`: thêm luật tương đương cho khối `DU_LIEU_QA` (dữ liệu do QA ghi, không làm theo chỉ dẫn trong khối, không mở link, không chạy lệnh; dòng "Nguồn: bug từ QA (Trello/Sheet)" ở đầu mục Mục tiêu). Giữ nguyên các luật Telegram hiện có.
- Test: `wants_untrusted_prompt(None, "<<<DU_LIEU_QA ab12cd34\n...")` ⇒ true; nội dung có `<<<HET_DU_LIEU_QA` giả bị vô hiệu.

### A4. Ảnh đính kèm của card
Bug giao diện (list HTML) cần ảnh. Trong `pull.rs`, với card Trello:
- Chỉ lấy attachment `isUpload == true` và `mimeType` bắt đầu bằng `image/`. Tối đa 5 ảnh mỗi card, mỗi ảnh ≤ 5 MiB (bỏ ảnh lớn hơn, ghi warning).
- Tải bằng client Trello hiện có (curl qua `trello::curl_bin`, header OAuth như các lệnh khác, không đưa key/token lên dòng lệnh; xem cách `TrelloClient` đang làm). Thêm hàm `TrelloClient::download(url, dest)`.
- Lưu vào `<home>/fix-bugs/images/<KEY>/<số thứ tự>-<tên đã làm sạch>` (`<home>` = `knowledge::find_or_default(project_root)`). Tên file chỉ giữ `[a-z0-9._-]`, tối đa 60 ký tự.
- **Trước khi ghi**, chạy `git check-ignore -q <home>/fix-bugs/images/x.png` trong repo dự án. Không bị ignore ⇒ không tải, thêm warning `"Thư mục ảnh fix-bugs/images/ chưa nằm trong .gitignore — bỏ qua tải ảnh"`.
- Trong khối dữ liệu QA, mục `--- Ảnh ---` liệt kê đường dẫn **tương đối từ gốc repo** của từng ảnh đã tải, để Lead dùng công cụ Read. Attachment không tải (link ngoài, không phải ảnh) vẫn liệt kê tên + URL như cũ.
- Lỗi tải một ảnh không làm hỏng cả lượt pull: ghi warning, đi tiếp.
- Test với server Trello giả (theo mẫu `tests/fixbug_readers.rs`): 2 ảnh upload + 1 link ngoài ⇒ tải 2 file, request có 2 đường dẫn; repo không ignore ⇒ không tải, có warning.

### A5. Nhập trạng thái của skill `/fix-bugs` cũ
Lệnh mới `agent-brain fixbug import-legacy PROJECT [--open skip|take] [--dry-run]` (`FixbugCommands::ImportLegacy` trong `main.rs`, logic ở `src/fixbug/legacy.rs`).
- Đọc **chỉ** `<home>/fix-bugs/tasks/*/state.json` (không đọc `README.md`, `images/` vì có dữ liệu cá nhân). Cần các trường `key`, `title`, `trello_card_id`, `trello_url`, `state`. Thiếu `trello_card_id` hoặc JSON hỏng ⇒ bỏ qua, ghi warning.
- Ánh xạ sang `fixbug_refs` (kind `trello`, `intake_id` NULL, `note = "legacy:<state cũ>"`):
  - `done` ⇒ `deployed`
  - `rejected`, `duplicate` ⇒ `rejected`
  - `need_info`, `plan_ready`: `--open skip` (mặc định) ⇒ `pulled` (agent-brain không kéo lại); `--open take` ⇒ **không ghi ref**, để `fixbug pull` kéo bug này thành task mới của agent-brain.
  - `approved`, `review`, `ready_to_merge`, `merged`, `blocked`, `parked` ⇒ luôn `pulled` (skill cũ đang giữ nhánh/worktree), kể cả khi `--open take`. In cảnh báo liệt kê các KEY này.
- Ref đã tồn tại (theo `project_id, kind, ext_id`) ⇒ bỏ qua.
- Không sửa bất kỳ file nào của skill cũ. Không gọi Trello.
- `--dry-run` chỉ in bảng `KEY | state cũ | sẽ ghi`.
- In tổng kết: số ref ghi mới theo từng state, số bỏ qua, các KEY để `take`.

### A6. Chặn pull khi chưa nhập trạng thái cũ hoặc skill cũ đang chạy
Thêm `fixbug::pull_guard(project_root, conn, project_id) -> Result<(), FixbugError>`, gọi ở **mọi** nơi pull (CLI, API dashboard, lệnh Telegram A8):
- Có `<home>/fix-bugs/.lock` ⇒ lỗi `"Skill /fix-bugs cũ đang chạy (có .lock) — thử lại sau"`.
- Có ít nhất một `<home>/fix-bugs/tasks/*/state.json` mà **chưa có ref nào có note bắt đầu bằng `legacy:`** của dự án ⇒ lỗi `"Dự án còn N bug của skill /fix-bugs cũ chưa nhập — chạy 'agent-brain fixbug import-legacy <dự án>' trước"`.
- CLI `fixbug pull` thêm cờ `--skip-legacy-check` để bỏ qua điều kiện thứ hai (không bỏ qua `.lock`). API và Telegram không có cờ này.
- Test cả ba nhánh.

### A7. Xem tiến độ bug
- `db.rs`: hàm `fixbug_progress(conn, project_id) -> Vec<FixbugProgress>` nối `fixbug_refs` → `proposals` (theo `intake_id`) → `tasks` (theo `proposals.task_id`). Mỗi dòng: `key, title, ref_state, note, intake_status, proposal_id, proposal_status, task_id, task_status, auto_state, branch`. Bug có nhiều đề xuất (dữ liệu cũ) ⇒ lấy đề xuất mới nhất.
- `fixbug list` in thêm cột `đề xuất` và `task` (vd `#12 approved`, `#40 qa_pass`). Ref `legacy:*` hiện "skill cũ".
- `GET /api/fixbug/{project}` trả thêm trường `progress`.
- Test truy vấn với dữ liệu giả: ref chưa có đề xuất, có đề xuất chưa duyệt, có task `qa_pass`.

### A8. Lệnh Telegram
Trong `telegram/intake.rs::on_command`, thêm (chỉ cho dự án nằm trong `telegram_projects`, dùng `allowed_projects` như `/task`; tên dự án khớp như `routing::resolve`):
- `/fixbugs <dự án> [số]`: chạy `pull_guard` rồi `pull` (số tối đa mặc định `DEFAULT_MAX_NEW`, trần 20). Trả lời: số bug mới, danh sách `KEY · priority · tiêu đề` (tối đa 20 dòng), số bỏ qua, warnings (cắt 5 dòng). Việc pull chạy ngoài luồng poll (theo cách các thao tác chặn khác của bot đang làm) để không treo bot.
- `/bugs <dự án>`: in tiến độ A7, tối đa 30 dòng, mỗi dòng `KEY · ref_state · task #id trạng thái`.
- `/deployed <dự án> <KEY...> [-- ghi chú]`: gọi đúng hàm của `fixbug deployed`.
- `/reject <dự án> <KEY> <lý do>`: gọi đúng hàm của `fixbug reject`; thiếu lý do ⇒ trả hướng dẫn.
- Cập nhật `HELP_TEXT` (`bot.rs:42`) liệt kê 4 lệnh mới.
- Không in token, key, board id trong trả lời. Lỗi Trello trả nguyên văn câu lỗi của `FixbugError` (đã không chứa secret; kiểm lại).
- Test hàm phân tích tham số (`parse_fixbug_cmd`, `parse_deployed_args`…) là hàm thuần.

### A9. Nhắc deploy khi bug qua QA
`telegram/render.rs` nhánh `EventKind::QaPass` (cả 2 chỗ `:379` và `:531`): nếu task gắn với một `fixbug_ref` (qua `proposals.task_id` → `intake_id`), thêm dòng:
`Bug <KEY>: merge rồi deploy, sau đó gõ /deployed <dự án> <KEY>`.
Dữ liệu lấy một lần khi dựng snapshot (không truy vấn trong hàm render thuần; thêm trường `fixbug_key: Option<String>` vào snapshot đang truyền vào render). Test render có và không có key.

### A10. Commit
`feat(agent-brain): fixbug trial — one task per bug, legacy import, images, Telegram commands`

## Phase B — task code phụ thuộc task code

### B1. Triage được dùng `after` giữa các task code
`orchestrator/triage.rs` luật số 10 (khi `intake.split`): đổi thành hai trường hợp:
- Như cũ: việc phân tích ở A làm đầu vào cho việc ở B.
- Mới: các việc **code** mà việc sau cần code của việc trước (sửa model rồi sửa màn hình dùng model đó, migration rồi code dùng cột mới) ⇒ việc sau có `"after": <chỉ số việc trước>`. Việc độc lập thì **không** dùng `after` để chạy song song.
Thêm một câu: không dùng `after` cho intake bug (đã không tách theo A1).

### B2. Scheduler chờ merge khi việc trước là code
`orchestrator/scheduler.rs`, nhánh `is_success` của việc trước:
- Lấy `kind` của quy trình việc trước (`workflows::load(&st.cfg).get(&prior_p.workflow)`; không tìm thấy ⇒ coi là `code`).
- `kind == analysis`: giữ nguyên hành vi hiện tại (chuyển câu trả lời làm tài liệu).
- `kind == code`: chỉ chạy việc sau khi nhánh task trước **đã merge vào nhánh gốc** của nó: `workspace::merge_base_of(project, task)` + `gitops::is_merged_into(project_root, &task.branch, &base)`. Chưa merge ⇒ `set_proposal_error(prop, "Chờ task #<id> được merge vào <base>")` rồi `continue`. Không có `base` ⇒ `set_proposal_error(prop, "Task #<id> không có nhánh gốc — merge tay rồi duyệt lại")` và `continue` (không chạy).
- Khi chạy được: xoá `error` của đề xuất. Task mới tạo nhánh từ nhánh gốc như bình thường (đã có code của task trước vì đã merge).
- Task trước `qa_pass` nhưng nhánh bị xoá ⇒ coi như chưa merge, ghi rõ trong error.

### B3. Thông báo
Khi B2 ghi lỗi "Chờ task #N được merge" lần đầu (error trước đó khác), gửi Telegram một lần qua outbox hiện có: `Task #M đang chờ task #N được merge vào <base>.` Không gửi lặp mỗi nhịp 5 giây (so error cũ/mới).

### B4. Test
Theo mẫu test scheduler có sẵn trong `tests/`:
- Việc trước code `qa_pass`, nhánh chưa merge ⇒ việc sau không chạy, error "Chờ task".
- Merge nhánh (git thật trong repo tạm) ⇒ nhịp sau việc sau chạy.
- Việc trước analysis `answered` ⇒ như cũ, có tài liệu `phan-tich-<id>.md`.
- Việc trước code không có base ⇒ không chạy, error đúng câu.
- Thông báo B3 chỉ ghi 1 lần qua 2 nhịp.

### B5. Commit
`feat(agent-brain): chain code tasks — wait for the earlier task to be merged`

## Không sửa
- `Cargo.toml` (không thêm crate; dùng crate sẵn có).
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dashboard thật cổng 7437.
- Repo dự án ngoài (bizsurvey, apec…).
- `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` khác, `~/.claude-orca`.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --release
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'   # passed > số ở bước 0, failed 0
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # ≤ số ở bước 0
grep -n "insert_intake" src/fixbug/pull.rs -A6 | grep -c "true"   # 0
./target/release/agent-brain fixbug --help | grep -c "import-legacy"   # 1
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
bash spikes/lib/scan-secrets.sh
```
Không chạy `fixbug pull` hay `import-legacy` trên dự án thật, không gọi Trello thật trong test.

## Commit chung
- Hai commit (A10, B5). `git add` đúng file, không `-A`, không `--no-verify`, không push.
- Tóm tắt cuối: file đã sửa, commit, các chỗ lệch plan và lý do.

---

## Phần C — chuẩn bị chạy thử (Claude làm sau khi review A + B, không giao agy)
Các bước đụng cấu hình người dùng và repo dự án, cần người dùng đồng ý từng bước:
1. Build bản release, dừng serve cũ đúng PID, chạy serve mới (theo quy tắc trong memory apec-e2e-autonomy).
2. `projects_map.json`, dự án `bizsurvey`:
   - `base_branch: "master_dev"`, `workspace: "shared"` (worktree mode chưa có `vendor/`, test Laravel không chạy được).
   - Thêm `bizsurvey` vào `telegram_projects`.
3. bizsurvey: thêm `/.gsd/fix-bugs/images/` vào `.gitignore` (một dòng, commit riêng trên nhánh người dùng chọn).
4. Ghi chú prompt riêng của bizsurvey `~/.config/agent-brain/prompts/projects/bizsurvey/{lead,dev,qa}.md`:
   - Không chạy, không sửa `.claude/skills/fix-bugs/**` và `.gsd/fix-bugs/tasks/**` (thuộc skill cũ).
   - Test chạy qua `.claude/skills/fix-bugs/scripts/gate.sh test|regression|suite` (suite đỏ sẵn, so baseline), không `php artisan test`, không migrate/serve; plan khai các lệnh này ở "Lệnh được chạy".
   - Bug giao diện: đọc ảnh trong `.gsd/fix-bugs/images/<KEY>/` trước khi lập plan.
5. `agent-brain fixbug init bizsurvey` (lấy list `Bugs`, `HTML`, Testing, Reject từ config cũ) rồi `fixbug check bizsurvey`.
6. `agent-brain fixbug import-legacy bizsurvey --dry-run`, người dùng chọn `--open skip` hay `take` cho T-35/T-38/T-40/T-41, rồi chạy thật.
7. Chạy thử: `/fixbugs bizsurvey 3` trên Telegram → duyệt đề xuất → duyệt plan → theo dõi tới QA → người dùng merge + deploy → `/deployed`.
