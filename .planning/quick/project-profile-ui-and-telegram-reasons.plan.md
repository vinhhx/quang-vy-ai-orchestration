# Plan: chọn profile dự án trên giao diện + bot Telegram nói rõ vì sao dự án không chạy được

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Lỗi thực tế (2026-10-29)
`/task video-srt …` trên Telegram trả `Không có dự án nào trong telegram_projects chạy được — kiểm tra id và đường dẫn trong projects_map.json.` Nguyên nhân: dự án `srt` có `"profile": ""`; `profile::check_profile` từ chối profile rỗng (`Dự án chưa đặt profile (work|personal) — …`), `intake::allowed_projects` nuốt lý do và chỉ trả thông báo chung.
Hai lỗi công cụ:
1. `projects::new_entry_from_projects` luôn đặt `profile: ""` và không có giao diện/API đặt profile; nút "Thêm dự án" trên màn Dự án (commit 1bda428) và `projects add` tạo dự án chưa chạy được cho tới khi sửa tay `projects_map.json`.
2. Thông báo Telegram không nói dự án nào bị loại và vì sao.

## Bối cảnh code (đã đọc)
- `src/profile.rs::check_profile(profile, &cfg.profiles)`: rỗng hoặc không có trong `cfg.profiles` (hiện `work`, `personal`) là lỗi.
- `src/projects.rs::require_runnable(project, cfg)` trả lỗi tiếng Việt cho: missing, path rỗng, thư mục không tồn tại, không phải repo git, profile sai. `intake::allowed_projects` (src/telegram/intake.rs ~dòng 307) gọi nó, bỏ lý do; hằng `NONE_RUNNABLE` (~dòng 59) và test dùng hằng này.
- `src/dashboard/api_projects.rs`: `update_aliases`, `update_telegram`, `add_project_api` (POST /api/projects), helper 409/lưu-và-trả-lời dùng chung; `project_json`.
- Giao diện: `assets/dashboard/js/projects.js`, `assets/dashboard/screens/projects.html`.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/dashboard/api_projects.rs` | route + hàm `update_profile` (`PUT /api/projects/{id}/profile`, body `{profile:string, etag}`, `deny_unknown_fields`) | Cùng mẫu khóa ghi + etag + 409 + 404. `profile` phải là khóa có trong `cfg.profiles` (lấy cấu hình qua cách các handler khác đang đọc `Config`; nếu AppState chưa có thì nạp bằng hàm sẵn có của `config.rs`, không nhân đôi logic), nếu không: 422 `Profile '<x>' không hợp lệ. Các profile đã cấu hình: work, personal`. Ghi `projects[i].profile`, lưu, `db::sync_projects`, trả cùng dạng thân như `update_aliases`. Không cho đặt chuỗi rỗng. | Đặt profile bằng giao diện |
| 2 | `agent-brain/src/dashboard/api_projects.rs` | `add_project_api` | Thêm trường tùy chọn `profile` vào thân `POST /api/projects` (kiểm như mục 1); nếu có thì ghi vào mục dự án vừa tạo (và cả dự án đã có nhưng đang rỗng profile). Không có thì giữ hành vi cũ (rỗng) nhưng phản hồi thêm `warnings: ["Dự án chưa có profile — đặt profile trước khi chạy task."]`. | Thêm dự án có profile ngay |
| 3 | `agent-brain/src/dashboard/api_projects.rs` | `project_json` | Thêm `"runnable_issue": string|null`: lý do ngắn tiếng Việt nếu dự án chưa chạy được vì lý do RẺ để kiểm (profile rỗng/không hợp lệ, missing, loại `media`); KHÔNG chạy git hay tạo shadow ở đây. | Giao diện cảnh báo sớm |
| 4 | `agent-brain/src/telegram/intake.rs` | `allowed_projects`, `NONE_RUNNABLE` | Thu lý do từng dự án bị loại: media, missing, và lỗi của `require_runnable` (cắt mỗi lý do ≤ 120 ký tự). Khi không còn dự án nào, trả `Err` có dạng `Không có dự án nào trong telegram_projects chạy được:\n• <id>: <lý do>\n• … (tối đa 10 dòng, thêm "… và N dự án khác" nếu dài)\nSửa trên màn Dự án của dashboard hoặc projects_map.json.` Giữ tiền tố `Không có dự án nào trong telegram_projects chạy được` để các test/so khớp cũ vẫn đúng; cập nhật hằng/test nào so khớp toàn câu cũ. Khi VẪN còn ít nhất một dự án chạy được thì hành vi cũ (không đổi). Không lộ đường dẫn tuyệt đối vào tin Telegram (chỉ id và lý do; nếu lý do chứa đường dẫn thì thay bằng tên thư mục cuối). | Thông báo rõ nguyên nhân |
| 5 | `agent-brain/assets/dashboard/js/projects.js` | trạng thái/phương thức | Thêm `setProfile(p, value)` gọi mục 1 (xử lý 409/422 như alias); `newProfile` cho ô thêm dự án (mặc định rỗng = "Chọn profile…", gửi kèm khi có); `profileOptions` lấy từ phản hồi (thêm `profiles:[...]` vào phản hồi `GET /api/projects` từ `cfg.profiles`, hoặc cố định nếu đã có sẵn nguồn trong `/api/projects`); hàm hiển thị `issueText(p)` từ `runnable_issue`. | Logic giao diện |
| 6 | `agent-brain/assets/dashboard/screens/projects.html` | thẻ dự án và khối "Thêm dự án" | Thẻ dự án: `<select>` (có nhãn "Profile") chọn work/personal, chọn thì gọi `setProfile`; hiện dòng cảnh báo `issueText(p)` khi có (ví dụ "Chưa đặt profile — chưa chạy được"). Khối thêm dự án: ô chọn profile. Chỉ dùng class Tailwind đã có trong `app.css`, KHÔNG thêm class mới, KHÔNG build lại CSS; không tràn ngang trên điện thoại, dùng bàn phím được, `min-h-12`. | Giao diện |
| 7 | `agent-brain/tests/dashboard_projects.rs` | cuối file | Test: (a) `PUT .../profile` hợp lệ ghi đúng, các khóa khác không đổi; (b) profile lạ → 422, rỗng → 422, id lạ → 404, etag cũ → 409, file không đổi khi lỗi; (c) `POST /api/projects` có `profile` ghi đúng, không có `profile` thì có `warnings`; (d) `runnable_issue` đúng cho dự án profile rỗng và media, `null` cho dự án hợp lệ. | Chốt hành vi |
| 8 | `agent-brain/tests/telegram_intake.rs` | cuối file | Test: danh sách chỉ có dự án profile rỗng → bot trả lời bắt đầu bằng `Không có dự án nào trong telegram_projects chạy được` và có dòng `• <id>: …chưa đặt profile…`; dự án thiếu thư mục → lý do tương ứng; không chứa đường dẫn tuyệt đối; khi có một dự án hợp lệ và một dự án lỗi thì vẫn hiện nút chọn dự án hợp lệ như cũ. Cập nhật test cũ so khớp thông báo cũ. | Chốt hành vi |
| 9 | `agent-brain/README.md` | mục API dự án, mục `telegram_projects` (~dòng 229) | Ghi `PUT /api/projects/{id}/profile`, trường `profile` của POST, `runnable_issue`, và thông báo mới liệt kê lý do từng dự án; ghi rằng dự án mới thêm mà chưa có profile thì chưa chạy được. | Tài liệu |

## Không được đụng tới
- `profile.rs::check_profile` và luật work/personal (SPK-10), `engine.rs`, quota, các vai, gitops (không tạo shadow khi liệt kê), `require_runnable` về hành vi.
- `assets/dashboard/app.css`, cấu hình Tailwind.
- `~/.config/agent-brain/*` thật: KHÔNG sửa `projects_map.json` của người dùng (người dùng tự chọn profile cho `srt`, `vietinbank` bằng giao diện mới); test dùng thư mục tạm.
- `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn. Không push.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -5
cargo test --offline --locked --test dashboard_projects --test telegram_intake --test dashboard_assets --test assets_parity 2>&1 | grep -E "^test result|FAILED"
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked"      # toàn bộ: không lỗi
node ../scripts/dashboard-js-check.mjs 2>&1 | tail -5
bash ../scripts/dashboard-ui-lint.sh 2>&1 | tail -3
cargo clippy --offline --locked --all-targets 2>&1 | grep -E "src/(dashboard/api_projects|telegram/intake)\.rs" | head
bash ../spikes/lib/scan-secrets.sh 2>&1 | tail -3
```
Acceptance:
1. Màn Dự án đổi được profile từng dự án và chọn profile khi thêm dự án; dự án chưa có profile có dòng cảnh báo.
2. `/task` khi không dự án nào chạy được trả danh sách từng dự án kèm lý do (không đường dẫn tuyệt đối); khi có dự án chạy được thì hành vi cũ.
3. Profile lạ/rỗng bị từ chối ở API; không khóa khác trong `projects_map.json` bị đổi.
4. Toàn bộ test cũ pass; chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Hai commit: `feat(agent-brain): choose project profile in the UI and explain Telegram refusals` (code, asset, test), `docs: project profile API and Telegram refusal reasons` (README). Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify, phần chưa kiểm (chưa bấm thử trên trình duyệt, chưa thử qua Telegram thật).
