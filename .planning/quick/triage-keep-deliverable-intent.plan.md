# Plan: việc "lấy dữ liệu / crawl / kiểm tra và giao kết quả" không bị phân loại thành phân tích hay thành "xây dựng chức năng"

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Vấn đề (task 57 và 58, 2026-11-12)
Người dùng gửi hai yêu cầu vào hàng chờ (màn Giao việc, `source = ui`) cho dự án `automation-test-e2e`:
- Intake 47: `Lấy toàn bộ sitemap của website này https://cnctech.com.vn/ … xuất ra thành list sitemap`. Bước phân loại (`orchestrator/triage.rs`) chọn quy trình `phan-tich`. Analyst chỉ đọc và không có mạng ("This command requires approval"), nên trả về bản ước lượng 10,5 giờ, không có URL nào (task 57).
- Intake 48: `Crawl website. Chú ý phân loại theo title … URL nào bị redirect về trang chủ … Xuất ra báo cáo`, kèm tài liệu dán `cnctech-sitemap.md` (111 ký tự, CHỈ là đường dẫn `~/.local/share/agent-brain/answers/automation-test-e2e/57-….md`, không phải nội dung). Bước phân loại viết lại `request` thành `Xây dựng chức năng crawl website … (website cụ thể chưa được nêu, cần xác nhận…)` và chọn quy trình `mac-dinh` (code). Lead lập plan làm một script chung nhận `--url`; QA đạt khi test của script pass (18/18). Script chưa bao giờ chạy trên website, không có danh sách hay báo cáo (task 58).

Nguyên nhân (đã đọc code):
1. `orchestrator/triage.rs`, hàm `build_triage_prompt`, quy tắc 4 (~dòng 494-496) chỉ có hai ngăn: phân tích/ước lượng/đánh giá → quy trình phân tích; sửa/làm code → quy trình code. Không có ngăn "thu thập/crawl/kiểm tra trên đích thật rồi giao kết quả".
2. Quy tắc 7 chỉ nói `request` là "mô tả chi tiết công việc cần làm"; agent điều phối tự do diễn đạt lại, và đổi ý định "lấy kết quả" thành "xây dựng chức năng". Không có luật giữ URL và tệp đính kèm người dùng đưa.
3. `assets/prompts/lead.md` ~dòng 24 ĐÃ có luật "yêu cầu là làm ra sản phẩm → `## Tạo sản phẩm` (chạy thật) và `## Sản phẩm` (file kết quả)", và `assets/prompts/qa.md` ~dòng 11 kiểm file sản phẩm thật sự tồn tại. Luật này không được dùng cho task 58 vì `request` đã bị viết thành "xây dựng chức năng". Luật cũng chưa nói rõ: yêu cầu lấy danh sách hoặc báo cáo từ một website/hệ thống thật là sản phẩm, kể cả khi phải viết công cụ trong cùng task.
4. Tài liệu dán chỉ chứa đường dẫn tới câu trả lời của task khác thì Lead không đọc được nội dung (file nằm ngoài worktree).

## Hiện trạng (đã đọc code)
- `agent-brain/src/orchestrator/triage.rs`: `build_triage_prompt` (~dòng 402-515): danh sách quy trình dạng `- tên, loại ("phân tích"|"code"), tóm tắt`; quy tắc đánh số 1-10; test prompt ở cuối file (~dòng 840+).
- `agent-brain/assets/prompts/lead.md` (~dòng 20-28 là các luật về sản phẩm, báo cáo, lệnh); `agent-brain/assets/prompts/qa.md` (~dòng 11).
- `agent-brain/src/inputs.rs`: `validate_name`, `validate_content(content, extra_secrets)`, `validate_inputs(items, secrets)`, `add_to_intake`, `add_to_task`, `add_to_proposal`; tài liệu nhận qua `dashboard/api_intake.rs::create_intake` (~dòng 86-231), `dashboard/api_tasks.rs` (~dòng 1116, 1253, 1335).
- `agent-brain/src/answers.rs`: `path(project_id, task_id, slug)` cho file trả lời; đây là thư mục trả lời của analyst.

## Luật mới
1. Prompt phân loại, quy tắc 4: thêm ngăn thứ ba. Yêu cầu **thu thập, crawl, lấy danh sách hoặc dữ liệu, chạy kiểm tra trên một website hoặc hệ thống thật rồi giao kết quả (danh sách, file, báo cáo)** KHÔNG phải phân tích (quy trình phân tích chỉ đọc code, không có mạng, chỉ giao tài liệu ước lượng) và KHÔNG chỉ là viết code: chọn quy trình loại code (ưu tiên quy trình có chữ `e2e` hoặc kiểm thử trong tên hay tóm tắt khi dự án có), và `request` phải nói rõ phải chạy thật trên đích đó và giao file kết quả.
2. Quy tắc về `request` (cạnh quy tắc 7): `request` giữ đúng ý định của người dùng. Nếu người dùng muốn KẾT QUẢ thì `request` mô tả việc lấy và giao kết quả đó; chỉ viết "xây dựng chức năng/công cụ" khi người dùng yêu cầu viết công cụ. Giữ nguyên mọi URL, tên file đính kèm và yêu cầu về định dạng xuất mà người dùng nêu; không thêm giả định thay cho thông tin thiếu: thông tin thiếu ghi thành câu hỏi trong `request` hay `reason`.
3. `lead.md`, ngay sau luật sản phẩm hiện có: yêu cầu lấy dữ liệu, danh sách hoặc báo cáo từ một website hay hệ thống thật là sản phẩm, kể cả khi task phải viết công cụ để lấy: plan có `## Tạo sản phẩm` chạy công cụ trên đích thật với tham số cụ thể (URL lấy từ yêu cầu hoặc tài liệu đính kèm), và `## Sản phẩm` liệt kê file kết quả; viết công cụ mà không chạy trên đích thật không phải là hoàn thành. Nếu đích bắt buộc (URL) không có ở yêu cầu hay tài liệu, ghi vào `## Vướng mắc` câu hỏi cần người dùng trả lời thay vì đặt tham số mặc định.
4. Tài liệu dán là một đường dẫn duy nhất tới file `.md` nằm trong thư mục `answers` (xem `answers.rs`) được thay bằng nội dung file đó lúc tiếp nhận, trước `validate_inputs`. Điều kiện chặt: nội dung (sau `trim`) là một dòng; là đường dẫn tuyệt đối; sau `canonicalize` nằm TRONG thư mục `answers` đã `canonicalize` (không theo symlink ra ngoài); đuôi `.md`; là file thường; kích thước không vượt giới hạn tài liệu hiện có; nội dung vẫn qua kiểm tra bí mật như mọi tài liệu. Không thoả điều nào thì giữ nguyên chuỗi người dùng dán (hành vi cũ). Tên tài liệu không đổi.

## Thay đổi
| # | File | Hàm / chỗ | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/orchestrator/triage.rs` | `build_triage_prompt`, quy tắc 4 và quy tắc 7; test cuối file | Thêm luật mới 1 và 2 vào chuỗi prompt (giữ đánh số liên tục: nếu thêm quy tắc mới thì dồn số, cập nhật `task_split_rule` nếu nó tham chiếu số quy tắc). Test: prompt dựng ra chứa cụm cốt lõi của từng luật mới (không so khớp toàn văn, chỉ vài cụm khoá như `thu thập`, `giao kết quả`, `giữ nguyên mọi URL`); các test prompt cũ vẫn pass. | Phân loại đúng và giữ ý định |
| 2 | `agent-brain/assets/prompts/lead.md` | ngay sau luật sản phẩm hiện có (~dòng 24) | Thêm luật mới 3. Giữ nguyên các dòng khác. Chạy `cargo test --offline --locked --test assets_parity` và mọi test đọc `lead.md` (`grep -rn "lead.md\|LEAD_PROMPT" tests src`). | Lead giao sản phẩm thật |
| 3 | `agent-brain/src/inputs.rs` | hàm mới `resolve_answer_ref(content: &str, answers_root: &Path) -> Option<String>` + gọi trước `validate_inputs` ở `create_intake` và hai chỗ nhận tài liệu của `api_tasks.rs` | Thực hiện luật mới 4. `answers_root` lấy từ chỗ `answers.rs` đang tính thư mục trả lời (thêm hàm `answers::root()` nếu cần, không nhân bản chuỗi đường dẫn). Hàm thuần theo tham số để test với thư mục tạm. Chỉ thay nội dung, không ghi đĩa, không đổi tên. | Lead đọc được kết quả của task trước |
| 4 | `agent-brain/tests/` (file test của hàng chờ/tài liệu đã có, ví dụ `task_inputs.rs` và `intake_api.rs`; hoặc `tests/answer_ref_input.rs` mới) | test | Thư mục tạm làm `answers`. Ca kiểm: (a) đường dẫn hợp lệ trong `answers` → nội dung file thay thế; (b) đường dẫn ngoài `answers` (`/etc/hostname`, `../` thoát ra) → giữ nguyên chuỗi; (c) symlink trong `answers` trỏ ra ngoài → giữ nguyên; (d) đuôi khác `.md` → giữ nguyên; (e) nhiều dòng hoặc có chữ khác kèm theo → giữ nguyên; (f) file quá giới hạn kích thước → giữ nguyên; (g) file chứa giá trị bí mật (giả lập qua `extra_secrets`) → bị từ chối như tài liệu thường; (h) `create_intake` qua HTTP với tài liệu là đường dẫn hợp lệ → tài liệu lưu là nội dung (đọc lại bằng `list_for_*`/`get_input_content`). Không dùng `~/.local/share/agent-brain` thật. | Chốt hành vi |
| 5 | `agent-brain/README.md` | mục giao việc / tài liệu đầu vào | Mô tả ngắn: dán đường dẫn file trong `answers/` thì dùng nội dung file; hàng chờ giữ ý định "lấy kết quả" và chạy thật; ví dụ một dòng. | Tài liệu |

## Không được đụng tới
- `orchestrator/scheduler.rs`, `db.rs`, `autopilot/*`, `dashboard/*` ngoài các điểm gọi `resolve_answer_ref` ở `api_intake.rs` và `api_tasks.rs`, `telegram/*`, `runner.rs`, `config.rs`.
- Các prompt khác (`lead-untrusted.md`, `dev.md`, `analyst.md`, `stacks/**`, `departments/**`): chỉ ĐỌC; nếu `lead-untrusted.md` lặp lại luật sản phẩm, ghi vào summary (không sửa).
- `~/.config/agent-brain/*` (kể cả `workflows.json`), `~/.local/share/agent-brain/*` (brain.db, runs, answers thật; serve đang chạy), `/mnt/data/source_codes/**`, `~/.claude-orca`, `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/` của việc khác.
- KHÔNG thay binary `agent-brain` đang chạy. KHÔNG dùng `pkill`/`killall`/kill theo tên tiến trình (CLAUDE.md của repo); chỉ dừng tiến trình do bạn chạy bằng PID bạn đã ghi. `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --lib triage 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --lib inputs 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --test assets_parity --test task_inputs --test intake_api --test orchestrator_triage 2>&1 | grep -E "^test result|FAILED|panicked"
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(orchestrator/triage|inputs)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -2
```
Test chập chờn đã biết (không phải của việc này; chạy lại riêng và ghi vào summary): `export_error_is_recorded_once_and_backs_off` trong `tests/journal_safety.rs`.

Acceptance:
1. Prompt phân loại có ngăn "thu thập/crawl/lấy dữ liệu và giao kết quả" và luật giữ ý định, URL, tên tệp đính kèm; test prompt cũ vẫn pass.
2. `lead.md` có luật mới; mọi test đọc `lead.md` pass.
3. Tài liệu dán là đường dẫn hợp lệ trong `answers` được thay bằng nội dung; mọi trường hợp ngoài điều kiện (ngoài thư mục, symlink thoát, sai đuôi, nhiều dòng, quá cỡ) giữ nguyên chuỗi cũ.
4. Chỉ các file trong bảng Thay đổi bị sửa (cộng hai điểm gọi ở `api_intake.rs`/`api_tasks.rs` và, nếu cần, một hàm trong `answers.rs`).

## Quy trình
- Hai commit: `feat(agent-brain): keep deliverable intent when triaging and let Lead deliver data from real targets` (mục 1, 2, 3, 4), `docs: deliverable requests and answer-path inputs` (mục 5). Mỗi commit kèm dòng `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify (số pass/fail), việc `lead-untrusted.md` có cần luật tương tự không, và phần chưa kiểm (chưa chạy phân loại với agent thật, serve chưa restart).
