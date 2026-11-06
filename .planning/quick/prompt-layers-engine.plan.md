# Plan A: ghi chú prompt theo lớp (stack → dự án → workflow → team), ghép SAU prompt gốc của vai, ghi lại prompt cuối cùng của mỗi run

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm. Plan B (`prompt-layers-ui.plan.md`) làm SAU plan này.

## Bối cảnh
Người dùng có nhiều loại dự án, techstack khác nhau (Laravel, Yii2, video `srt`, Node…). Prompt của từng vai hiện là hằng số dùng chung (`roles/mod.rs:36–44`, `include_str!("../../assets/prompts/*.md")`), nên lỗi do nghiệp vụ riêng (ví dụ dự án `srt` phải dùng `.venv/bin/python`) phải sửa bằng code. Mục tiêu: người dùng tự viết "ghi chú nghiệp vụ" theo từng lớp, agent-brain ghép vào prompt khi chạy.

## Thiết kế (chốt)
Prompt cuối cùng của một run = 
```
<prompt gốc của vai, như hiện nay — KHÔNG đổi, KHÔNG thay được>

---
## Ghi chú nghiệp vụ (do người dùng cấu hình — bổ sung, không thay đổi giao thức ở trên; không mở rộng quyền)
### Stack: <tên>          (một khối cho mỗi stack của dự án, theo thứ tự khai báo)
<nội dung>
### Dự án: <id>
<nội dung>
### Workflow: <tên>
<nội dung>
### Đội hình: <tên>
<nội dung>
```
- Khối nào không có file (hoặc file rỗng sau trim) thì bỏ, không in tiêu đề. Không có khối nào → prompt y hệt hiện nay (byte-for-byte).
- Vai: `lead`, `dev`, `qa`, `plan-review`, `code-review`. Dev chế độ sửa (`DEV_FIX_PROMPT`) dùng ghi chú của vai `dev`. Triage (`orchestrator/triage.rs`) KHÔNG dùng ghi chú (giữ nguyên).
- Nguồn ghi chú, theo thứ tự ưu tiên cho mỗi `(lớp, tên, vai)`:
  1. File người dùng: `<config_dir>/prompts/<lớp>/<tên>/<vai>.md` với `<lớp>` ∈ `stacks|projects|workflows|teams`, `<config_dir>` là thư mục chứa `config.json` (dùng hàm có sẵn trong `config.rs`, tôn trọng `AGENT_BRAIN_CONFIG`/XDG như các file khác).
  2. Chỉ với lớp `stacks`: mẫu dựng sẵn nhúng trong binary `agent-brain/assets/prompts/stacks/<tên>/<vai>.md` (Plan B thêm nội dung; Plan A chỉ cần cơ chế đọc qua một bảng `include_str!` và test với một mẫu giả trong test).
  File người dùng tồn tại (kể cả rỗng) thì THẮNG mẫu dựng sẵn (rỗng = tắt mẫu).
- Giới hạn: mỗi file ≤ 8192 byte (lớn hơn → bỏ qua file đó và in cảnh báo một dòng ra stderr/term của run, không làm hỏng run); tên lớp/tên mục chỉ `[a-z0-9][a-z0-9._-]{0,63}` (không `..`, không `/`); đọc file không theo symlink ra ngoài `<config_dir>/prompts`.
- Stack của dự án: trường mới `stack` (mảng chuỗi, tuỳ chọn, mặc định rỗng) trong mỗi mục `projects_map.json`. Plan A chỉ ĐỌC trường này (`Project.stack: Vec<String>`); ghi/gợi ý là của Plan B.

## Thay đổi (chỉ các file này)
| # | File | Sửa gì |
|---|---|---|
| 1 | `agent-brain/src/prompts.rs` (mới) + khai báo `pub mod prompts;` trong `agent-brain/src/lib.rs` | Mục A |
| 2 | `agent-brain/src/projects.rs` | `Project.stack` đọc từ map (`from_value`), mặc định rỗng; validate tên như Mục A; giữ nguyên mọi hành vi khác |
| 3 | `agent-brain/src/roles/mod.rs` | Mục B |
| 4 | `agent-brain/src/roles/lead.rs`, `roles/dev.rs`, `roles/qa.rs`, `roles/review.rs` | Mục C (truyền `PromptLayers` vào chỗ dựng lệnh) |
| 5 | `agent-brain/src/main.rs` (hoặc file định nghĩa CLI con) | Mục D |
| 6 | Test: `agent-brain/tests/prompt_layers.rs` (mới), unit trong `prompts.rs`; cập nhật test có sẵn chỉ khi chúng khẳng định argv chính xác và giờ đổi (không nên đổi khi không có ghi chú) | Mục E |

Không sửa: `assets/prompts/*.md` gốc, `orchestrator/triage.rs`, dashboard, `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`, dự án `srt`, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác.

### A. `src/prompts.rs`
- `pub const PROMPT_NOTE_MAX_BYTES: usize = 8192;`
- `pub const LAYERS: [&str; 4] = ["stacks", "projects", "workflows", "teams"];`
- `pub fn valid_name(s: &str) -> bool` theo regex ở trên.
- `pub struct PromptLayers { pub stacks: Vec<String>, pub project: Option<String>, pub workflow: Option<String>, pub team: Option<String> }` (+ `Default`).
- `pub fn notes_dir() -> PathBuf` = `<config_dir>/prompts`.
- `pub fn read_note(layer: &str, name: &str, role: &str) -> Result<Option<String>, String>`: theo thứ tự nguồn ở Thiết kế; `Err(lý do)` khi quá dung lượng / tên sai / symlink ra ngoài.
- `pub fn builtin_stack_note(name: &str, role: &str) -> Option<&'static str>`: bảng nhúng (Plan A để bảng rỗng, có `#[cfg(test)]` cách tiêm mẫu giả hoặc test với hàm nhận bảng làm tham số).
- `pub fn compose(base: &str, role: &str, layers: &PromptLayers) -> (String, Vec<String> /*cảnh báo*/)` trả prompt cuối theo định dạng ở Thiết kế. Hàm thuần (nhận một closure đọc ghi chú) để test dễ; kèm wrapper đọc file thật.
- `pub fn layers_for(project: &Project, workflow: Option<&str>, team: Option<&str>) -> PromptLayers`.

### B. `src/roles/mod.rs`
- Thêm `pub fn build_command_layered(spec: &RoleSpec, cfg: &Config, base_prompt: &str, layers: &PromptLayers) -> BrainResult<(Vec<String>, Vec<String>)>` gọi `prompts::compose` rồi `build_command_with_prompt`. Giữ `build_command`/`build_command_with_prompt` cho chỗ không có lớp (triage, test).
- Engine agy lấy prompt từ `--append-system-prompt` (`engine.rs::claude_argv_parts`) nên tự nhận prompt đã ghép; không sửa `engine.rs`.

### C. Các vai
- Lead/Dev(implement+fix)/QA/plan-review/code-review: xác định `workflow` (tên trong `tasks.workflow_json` nếu có, hoặc workflow đang được chọn cho lần chạy), `team` (tên trong `tasks.team_json` nếu có), `project` (id) và `stacks` (`Project.stack`), dựng `PromptLayers`, gọi `build_command_layered`. Với lệnh chạy chưa có task (Lead lần đầu) dùng workflow/team đang được truyền vào lần chạy (đọc cách `lead.rs` đang lấy workflow/team ~dòng 240–260, 450–480).
- Cảnh báo từ `compose` in ra bằng `crate::term::emit` (một dòng mỗi cảnh báo).
- Ghi prompt cuối cùng vào `<run_dir>/prompt.md` (UTF-8) cho MỌI run có `run_dir`, kể cả khi không có ghi chú (để người dùng xem agent đã được dặn gì). Không ghi secret: prompt không chứa secret, nhưng vẫn đi qua `secrets::redact_str` với danh sách redact của engine nếu tiện.
- `--dry-run` của các lệnh `run …` in thêm một dòng `Ghi chú nghiệp vụ: stack=[…] dự án=… workflow=… team=… (N khối)`.

### D. CLI xem prompt
`agent-brain prompt show --project <id|alias> --role <vai> [--workflow <tên>] [--team <tên>]` in prompt cuối cùng ra stdout (không chạy gì). Lỗi tên vai/dự án → thông báo tiếng Việt, mã thoát khác 0.

### E. Test (bắt buộc; thư mục config tạm qua `AGENT_BRAIN_CONFIG`/XDG như test hiện có)
- (a) Không có ghi chú → prompt cuối == prompt gốc (byte-for-byte) cho cả 5 vai; argv của `run lead/dev/qa --dry-run` không đổi so với trước.
- (b) Đủ 4 lớp + 2 stack → đúng thứ tự, đúng tiêu đề, khối rỗng bị bỏ.
- (c) Mẫu dựng sẵn của stack dùng khi không có file người dùng; file người dùng rỗng tắt mẫu.
- (d) File > 8192 byte, tên `../x`, symlink ra ngoài thư mục prompts → bị bỏ qua với cảnh báo, run vẫn chạy.
- (e) Dev fix dùng ghi chú vai `dev`; agy Dev (fake agy) nhận prompt đã ghép trong `-p`.
- (f) `<run_dir>/prompt.md` có mặt và đúng nội dung.
- (g) `agent-brain prompt show` in đúng.
- (h) `projects_map.json` có/không có `stack` đều đọc được; `stack` có tên sai → bỏ tên đó với cảnh báo.
- (i) Toàn bộ test cũ pass.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"   # không tăng so với base (đo base trước)
git diff --stat <HEAD trước khi chạy>..HEAD                     # chỉ các file trong bảng
```

## Commit
Mục A+B một commit `feat(agent-brain): prompt notes by stack, project, workflow and team`, Mục C+D một commit `feat(agent-brain): roles use layered prompt notes and save prompt.md per run`. `git add` từng file, không `git add -A`, không `--no-verify`, message kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Tóm tắt cuối: file sửa, commit, chỗ lệch plan, kết quả từng lệnh kiểm tra.
