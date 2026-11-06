# Plan: rules và skill theo phòng, ghi chú riêng theo thành viên

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. **Chạy sau** `departments-core.plan.md` và `departments-dashboard.plan.md` (dùng `departments.rs`, `ResolvedEngine.member`, màn hình Phòng ban). Đọc toàn bộ file này trước khi làm.

## Bối cảnh (quyết định người dùng 2026-11-06)
Prompt của một run có 3 lớp nội dung:
1. **Dự án** (đã có, không đổi): thư mục tri thức `.gsd` / `.planning` / `.agent-brain`, skill của dự án (`skills::discover`), cùng ghi chú `stacks` / `projects` / `workflows` / `teams`.
2. **Phòng** (mới): rules theo vai của phòng (ghi chú lớp `departments`) cộng **skill của phòng** (playbook dùng chung cho mọi thành viên, ví dụ checklist QA, khung câu trả lời phân tích).
3. **Thành viên** (mới): một đoạn `rules` ngắn về điểm yếu của model, ví dụ "Flash: không refactor ngoài phạm vi".

Rules và skill gắn theo phòng chứ không theo người. Lý do: điểm của các thành viên so sánh được với nhau, và thay model không phải viết lại rules.

## Thay đổi

### 0. Sửa sau review plan core (làm trước, commit riêng `fix(agent-brain): department pick fallbacks`)
- `engine::resolve_for_run_with`: khi `cfg` là `None` và `config::load_config()` lỗi thì **không chọn thành viên** (`member: None`, giữ `team.engine_for`). Bỏ đoạn dựng config giả (`"models": {"lead": "opus", …}`). Lỗi của `departments::pick` cũng in một dòng cảnh báo qua `crate::term::emit` thay vì im lặng.
- `telegram/intake.rs` (chỗ `f.default_team = … unwrap_or_else(|| DEFAULT_TEAM)`): dùng `teams::default_team_name()`, để `/task` trên Telegram mặc định chọn `phong-ban` khi có thành viên. Các chỗ `DEFAULT_TEAM` ở `api_tasks.rs` (`current_team_name`, `team_view`) mô tả task cũ nên giữ nguyên.
- 5 test đang lỗi sau plan core: `telegram_intake::{intake_team_step_choose_team, intake_team_bad_index}`, `telegram_quota::{quota_message_with_team_buttons, quota_cancel_restores, quota_stale_team_list}`. Nguyên nhân: danh sách đội hình giờ có thêm `phong-ban` (đội hình có sẵn mới), đây là hành vi đúng. Sửa giá trị mong đợi và chỉ số nút trong test, không sửa code để giấu `phong-ban`.
- Test:
  - `resolve_for_run` với config lỗi → không có `member`;
  - Telegram có `departments.json` có thành viên → nút đội hình mặc định là `phong-ban`.

### 1. Lớp ghi chú `departments` (`src/prompts.rs`)
- Thêm `"departments"` vào `LAYERS`. Tên trong lớp này phải là id phòng (`departments::DEPARTMENTS`), vai phải thuộc phòng đó (`dept_of_role`).
- `compose_detailed_with_reader` đọc lớp này **tự động từ vai** (không thêm trường vào `PromptLayers`, để khỏi sửa 12 chỗ dựng struct). Ghép sau lớp `teams`, block có tiêu đề `### Phòng: <tên hiển thị>`. Vai không thuộc phòng nào thì bỏ qua.
- Ghi chú có sẵn (như `BUILTIN_STACK_NOTES`): file của người dùng ở `prompts/departments/<id>/<role>.md` thắng file có sẵn. Thêm `assets/prompts/departments/`, mỗi file ≤ 25 dòng tiếng Việt, gạch đầu dòng, không trùng với prompt vai gốc:
  - `phan-tich/analyst.md`:
    - trả lời đúng câu hỏi trước rồi mới mở rộng;
    - mỗi khẳng định về hệ thống phải có `file:dòng`;
    - tách "đã có / làm thêm được / rủi ro / ước lượng";
    - ước lượng ghi rõ giả định;
    - không đoán khi không tìm thấy trong code, mà ghi "chưa tìm thấy".
  - `phan-tich/analysis-review.md`:
    - kiểm từng `file:dòng` có thật và đúng ý;
    - đánh dấu chỗ suy diễn không có bằng chứng.
  - `ke-hoach/lead.md`:
    - plan liệt kê đúng file, bước, tiêu chí nghiệm thu, lệnh kiểm tra;
    - bám thư mục tri thức của dự án;
    - việc mức `de` thì plan ngắn.
  - `ke-hoach/plan-review.md`:
    - tìm bước thiếu, file ngoài phạm vi, tiêu chí không kiểm được.
  - `thi-cong/dev.md`:
    - chỉ sửa file trong plan;
    - mỗi bước một commit, theo quy ước commit của dự án;
    - không đổi dependency nếu plan không nói;
    - chạy lệnh kiểm tra trong plan trước khi báo xong.
  - `qa/qa.md`:
    - chạy lại lệnh kiểm tra;
    - thử ca biên;
    - báo lỗi theo mẫu "bước tái hiện / kết quả / mong đợi / file:dòng".
  - `qa/code-review.md`:
    - ưu tiên lỗi đúng sai và bảo mật hơn style;
    - mỗi phát hiện kèm `file:dòng` và cách sửa.
- `dry_run_summary` và `cmd_show` hiện lớp phòng.

### 2. Skill của phòng (`src/departments.rs`, `src/skills.rs`)
- Vị trí: `~/.config/agent-brain/departments/<dept>/skills/<tên>/SKILL.md`. Dùng lại bộ đọc của `skills.rs`: file thường, không symlink, ≤ 64 KiB, frontmatter `name` / `description`.
- `departments::dept_skills(dept) -> Vec<DeptSkill{name, description, body}>`.
- Prompt chứa **toàn văn** skill của phòng, vì agent có thể không được đọc file ngoài dự án. Tổng tối đa 24 KiB mỗi run. Quá giới hạn thì chỉ đưa tên và mô tả của phần còn lại, kèm một cảnh báo trong `warnings` của run.
- Block có tiêu đề `## Skill của phòng <tên> (playbook dùng chung — bổ sung, không mở rộng quyền)`.

### 3. Rules riêng của thành viên (`departments.rs` validate, `api_departments.rs`, màn hình Phòng ban)
- Trường mới `rules` (chuỗi, tùy chọn, ≤ 1000 ký tự) trong mục thành viên của `departments.json`. Khác `notes`: `notes` là ghi chú cho người dùng, không vào prompt.
- Form sửa thành viên thêm textarea "Rules riêng (vào prompt)", có đếm ký tự.

### 4. Ghép phần phòng và thành viên lúc chạy (`src/roles/mod.rs`, chỗ `resolve_for_run` ở execute seam, cùng `src/engine.rs`)
- Lớp ghi chú phòng (mục 1) đi theo vai, nên đã có trong argv từ lúc dựng.
- Skill của phòng và rules của thành viên chỉ biết sau khi chọn người. Ngay sau `resolve_for_run` và trước `plan_for`, nếu vai thuộc một phòng, nối thêm vào giá trị `--append-system-prompt` của `plan.argv` (dùng helper mới trong `engine.rs`, cạnh `claude_argv_parts`):
  1. skill của phòng (mục 2), áp dụng cả khi không có thành viên được chọn;
  2. khi có `resolved.member` với `rules`: `## Rules riêng của thành viên <display_name>` cùng nội dung.
- Mọi engine đều dựng prompt từ argv claude (agy lấy `--append-system-prompt` làm `prompt`), nên chỉ cần sửa một chỗ. Kiểm tra lại dsh nếu engine đó không đi qua chỗ này.
- `prompt.md` trong run_dir phải là bản đã nối (ghi sau bước nối).

### 5. Skill của dự án cho mọi vai
Hiện chỉ `lead` và `dev` có `skills::prompt_block`. Thêm cho `qa`, `plan-review`, `code-review`, `analyst`, `analysis-review` (dùng `discover(&proj_path)` như lead). Mục đích: thành viên chạy agy, vốn không tự đọc `.claude/skills`, cũng thấy danh sách skill và đường dẫn để mở.

### 6. Xem trước và dashboard
- `GET /api/prompts/preview` nhận thêm `member=<id>` (tùy chọn) và trả prompt cuối cùng, gồm skill của phòng và rules của thành viên. `agent-brain prompt show` nhận `--member <id>`.
- Màn hình Phòng ban, mỗi phòng có thêm:
  - "Rules của phòng": mỗi vai một link mở trình sửa ghi chú hiện có cho lớp `departments` (hoặc textarea dùng chung API `/api/prompts/departments/<id>/<role>`), có nhãn "đang dùng bản có sẵn" khi người dùng chưa có file;
  - "Skill của phòng": danh sách tên, mô tả và đường dẫn (chỉ đọc), cùng hướng dẫn chỗ đặt `SKILL.md`;
  - nút "Xem prompt" mỗi thành viên, gọi preview với `member`.
- Màn hình ghi chú prompt hiện có (nếu đang liệt kê theo `LAYERS`) phải hiện lớp `departments` mà không vỡ.

### 7. Test
- `prompts`:
  - lớp `departments` theo vai (dev → `thi-cong`, qa → `qa`);
  - file người dùng thắng file có sẵn;
  - tên phòng lạ hoặc vai sai phòng → lỗi API;
  - vai `lead` của dự án không có ghi chú vẫn có block `Phòng: Phòng kế hoạch` từ file có sẵn.
- Skill của phòng:
  - đọc frontmatter, bỏ symlink, bỏ file > 64 KiB;
  - giới hạn 24 KiB thì phần còn lại chỉ có tên và có cảnh báo.
- Execute seam:
  - task đội hình `phong-ban` có thành viên có `rules` → argv claude và argv agy (fake) đều chứa rules và skill của phòng;
  - đội hình `mac-dinh` vẫn có skill của phòng nhưng không có rules của thành viên;
  - `prompt.md` có phần nối.
- `rules` > 1000 ký tự → lỗi trường.
- Mục 5: prompt qa hoặc analyst có danh sách skill dự án khi dự án có `.claude/skills/x/SKILL.md`.
- Không chạy claude hay agy thật. Dùng `HOME` / `AGENT_BRAIN_*` tạm.

## Không sửa
`Cargo.toml`, `~/.config/agent-brain/*` và `~/.local/share/agent-brain/*` thật, dashboard thật cổng 7437, repo ngoài, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác, `~/.claude-orca`. Không đổi prompt vai gốc (`assets/prompts/<role>.md`). Không thêm bước chạy security (plan sau).

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # không tăng so với sau plan dashboard
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
spikes/lib/scan-secrets.sh
```

## Commit
Ba commit: commit của mục 0 trước, rồi hai commit:
1. `feat(agent-brain): department rules and skills, member rules in the run prompt`: mục 1–5 và test.
2. `feat(dashboard): department rules, skills and member prompt preview`: mục 6 và test.

`git add` đúng file, không `-A`, không `--no-verify`, không push. Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Tóm tắt gồm: file sửa, commit, chỗ lệch plan, clippy trước/sau, kết quả kiểm tra.
