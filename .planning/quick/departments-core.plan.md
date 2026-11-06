# Plan: phòng ban, thành viên và chọn người theo độ khó (lõi)

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main` (f77b2d9 trở lên). Đọc toàn bộ file này trước khi làm.

## Bối cảnh (quyết định người dùng 2026-11-06)
- Bỏ cách nghĩ "đội hình = mỗi vai một engine cố định". Người dùng tổ chức thành **phòng ban**, mỗi phòng có nhiều **thành viên**. Mỗi thành viên là một engine kèm model do người dùng tự chọn, và tự gán **mức độ khó** mà thành viên đó nhận.
- Mỗi task có **độ khó** `de` | `vua` | `kho`. Triage gán độ khó, người dùng sửa được. Khi chạy một vai, agent-brain chọn thành viên của phòng phụ trách vai đó theo độ khó.
- Người dùng sẽ chấm điểm thành viên trên dashboard (plan `departments-dashboard`). Plan này tạo sẵn bảng điểm và dùng điểm khi chọn người.
- Phòng Security chỉ khai báo thành viên ở plan này. Bước chạy security thuộc plan sau.

## Mô hình
| Phòng (id) | Tên hiển thị | Vai do phòng đảm nhận |
|---|---|---|
| `phan-tich` | Phòng phân tích | `analyst`, `analysis-review` |
| `ke-hoach` | Phòng kế hoạch | `lead`, `plan-review` |
| `thi-cong` | Phòng thi công | `dev` |
| `qa` | Phòng QA | `qa`, `code-review` |
| `bao-mat` | Phòng security | (chưa có vai, plan sau) |

Năm phòng là danh sách cố định trong code. File chỉ khai báo thành viên.

File `~/.config/agent-brain/departments.json` (giới hạn 256 KiB, như `teams.json`):
```json
{
  "phan-tich": {
    "members": {
      "opus-1": { "display_name": "Senior Opus 1", "engine": "claude", "model": "claude-opus-5-5", "levels": ["vua", "kho"] },
      "argon":  { "display_name": "Argon", "engine": "agy", "model": "gemini-4-argon-high", "levels": ["vua", "kho"], "notes": "..." }
    }
  }
}
```
- Khóa ngoài cùng phải là một trong năm id phòng. Khóa lạ là lỗi.
- Mã thành viên dùng luật slug của `staff::validate_id`, duy nhất **trên toàn file** (không chỉ trong một phòng), vì `runs.member_id` lưu mã trần.
- `engine`: `claude` hoặc một engine id có trong `config.json` `engines`. Engine `người:` và engine kiểu `dsh` bị từ chối.
- `model`: chuỗi tùy chọn, tối đa 64 ký tự, gồm chữ, số và `.-_:/`. Không có thì dùng model mặc định của engine cho vai.
- `levels`: mảng con không rỗng của `["de","vua","kho"]`, không trùng.
- `display_name` bắt buộc (≤ 64 ký tự), `notes` tùy chọn (≤ 500).
- File không có nghĩa là không có thành viên, và tính năng tắt. Lỗi từng thành viên được gom vào `errors` như `teams::load_from` (thành viên lỗi bị bỏ qua, phần còn lại vẫn dùng).

## Thay đổi

### 1. Module `agent-brain/src/departments.rs` (mới) và `lib.rs`
- Có `DEPARTMENTS` (id, tên hiển thị), `dept_of_role(role) -> Option<&'static str>`, `Level { De, Vua, Kho }` (parse/as_str), các struct `Member { id, department, display_name, engine: Engine, model: Option<String>, levels: Vec<Level>, notes }`, `LoadedDepartments { members, errors }`, và các hàm `departments_path()`, `load_from(text, cfg)`, `load_path`, `load()`, `has_members()`. Tách validate thành `validate_entry` để plan dashboard dùng lại.
- `model_family(model) -> String`: viết thường, bỏ đuôi `-low` / `-medium` / `-high`. Ví dụ `claude-opus-5-5-high` và `claude-opus-5-5` cùng họ.
- `pick(conn, cfg, task_id, role, level, project_profile) -> BrainResult<Option<Pick>>`, với `Pick { member: Member, reason: String }`. Luật chọn:
  1. Vai không thuộc phòng nào, hoặc phòng không có thành viên → `None` (dùng đội hình như cũ).
  2. **Giữ người cũ:** task đã có run cùng vai với `member_id` còn trong file, engine sẵn sàng và có mức độ khó tương ứng → chọn lại thành viên đó. Ví dụ bước `fix` dùng lại người đã code.
  3. Ứng viên là thành viên của phòng có `level` của task. Không có ai thì lấy mọi thành viên của phòng. Bỏ người có engine không sẵn sàng (`engine::readiness`) hoặc bị `engine::profile_refusal` từ chối.
  4. **Độc lập khi review:**
     - Vai `plan-review` → so với thành viên của run `lead` thành công gần nhất của task.
     - Vai `code-review` hoặc `qa` → so với run `dev` gần nhất.
     - Vai `analysis-review` → so với run `analyst` gần nhất.

     Nếu có ứng viên khác `model_family` (họ model lấy từ `runs.model`) thì chỉ giữ những người đó. Không có thì giữ tất cả.
  5. Xếp hạng theo thứ tự:
     1. Ít run `running` nhất (theo `runs.member_id`).
     2. Điểm trung bình cao nhất. Thành viên có dưới 5 điểm tính là 3.0.
     3. Thứ tự trong file.
  6. `reason` là câu ngắn tiếng Việt, ví dụ `"phòng thi công · mức vừa · ít việc nhất"`.

### 2. Lược đồ: migration mới (`db.rs`, `SCHEMA_VERSION` 7 → 8)
- `ALTER TABLE tasks ADD COLUMN level TEXT NOT NULL DEFAULT 'vua' CHECK (level IN ('de','vua','kho'))`. Thêm cột tương tự vào `proposals`.
- `ALTER TABLE runs ADD COLUMN member_id TEXT`.
- Bảng `member_scores (id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL UNIQUE REFERENCES runs(id), member_id TEXT NOT NULL, task_id INTEGER REFERENCES tasks(id), score INTEGER NOT NULL CHECK (score BETWEEN 1 AND 5), note TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)` và chỉ mục theo `member_id`.
- Hàm db cho các bước trên: `set_task_level`, `task_level`, `member_score_avg(member_id) -> (avg, count)`, `running_count_by_member`, `last_member_for(task, role)`.
- Thêm test vào `tests/db_migration.rs`: nâng cấp từ 7 lên 8 giữ nguyên dữ liệu, và cột mặc định là `vua`.

### 3. Đội hình có sẵn `phong-ban` và chọn engine (`teams.rs`, `assets/teams.default.json`, `engine.rs`)
- Thêm đội hình có sẵn `phong-ban`: mọi vai là `claude`, mô tả "Chọn thành viên theo phòng ban và độ khó (departments.json); vai chưa có thành viên chạy claude."
- Đội hình mặc định (`DEFAULT_TEAM` ở các chỗ đang dùng) đổi thành hàm `teams::default_team_name()`. Hàm trả `phong-ban` khi `departments::load().has_members()`, ngược lại trả `mac-dinh`. Giữ hằng `DEFAULT_TEAM = "mac-dinh"` cho chỗ cần tên dự phòng.
- `engine::ResolvedEngine` thêm `member: Option<departments::Pick>`. Trong `resolve_for_run`:
  - Khi đội hình của task là `phong-ban` và vai đó **không** đang chờ người thật, gọi `departments::pick` với `tasks.level`.
  - Có kết quả thì `engine = member.engine`, ngược lại giữ `team.engine_for`.
  - Đội hình khác không đổi gì.
- `plan_for`: khi `resolved.member` có `model`, model đó thắng mọi cấu hình khác:
  - claude → thay giá trị sau `--model` trong argv (hoặc thêm vào).
  - `anthropic_plan` → dùng trước `a.models`.
  - agy → dùng trước `acfg.models` và `acfg.model`.

  `model_label` hiện đúng model đó.
- Nơi chèn dòng `runs` ghi `member_id`. Dòng tiêu đề của run (chỗ in engine và model) thêm `· <display_name>`.

### 4. Độ khó trong luồng việc (`orchestrator/triage.rs`, `api_tasks.rs::start_new_task`, `telegram/intake.rs`, CLI)
- JSON của triage có thêm `"level"`. Kiểm tra như `priority`: thiếu thì mặc định `vua`, giá trị lạ là lỗi. Prompt triage thêm luật:
  - `de`: sửa chữ, cấu hình, một file, bug rõ nguyên nhân.
  - `vua`: tính năng hoặc bug vài file trong một module.
  - `kho`: nhiều module, kiến trúc, migration dữ liệu, bảo mật, hoặc nguyên nhân chưa rõ.
- `proposals.level` → `tasks.level` khi `start_new_task`. Tin Telegram đề xuất task hiện `Độ khó: Dễ/Vừa/Khó` ngay cạnh chỗ đang hiện mức ưu tiên.
- CLI: lệnh tạo task, `run lead` và `run analyst` nhận `--level de|vua|kho`.

### 5. CLI `agent-brain dept` (`main.rs`)
- `dept list`: mỗi phòng một khối, mỗi thành viên một dòng gồm mã, tên, engine, model, mức độ, độ sẵn sàng của engine, điểm TB (số lượt), số run. Cuối khối in các lỗi đọc file.
- `dept check`: in lỗi và thoát mã 1 nếu file có lỗi. Không có file thì in đường dẫn và hướng dẫn chép file mẫu.
- `dept pick <task_id> <role>`: chạy thử, in người được chọn và `reason`, không chạy gì.

### 6. File mẫu `agent-brain/assets/departments.example.json` (không nhúng vào binary)
Đội hình của người dùng. Mức độ là gợi ý, người dùng tự sửa. Model claude dùng id của Claude CLI, model agy dùng id từ `agy models`.
| Phòng | Thành viên (mã: engine / model / levels) |
|---|---|
| phan-tich | `opus-1`, `opus-2`, `opus-3`: claude / claude-opus-5-5 / vua,kho · `argon`: agy / gemini-4-argon-high / vua,kho · `flash`: agy / gemini-3.8-flash-medium / de,vua · `haiku`: claude / claude-haiku-5-5 / de |
| ke-hoach | `kh-opus`: claude / claude-opus-5-5 / kho · `kh-sonnet`: claude / claude-sonnet-5-5 / de,vua · `kh-argon`: agy / gemini-4-argon-high / vua,kho |
| thi-cong | `tc-sonnet-1`, `tc-sonnet-2`: claude / claude-sonnet-5-5 / vua,kho · `tc-flash-1`, `tc-flash-2`: agy / gemini-3.8-flash-high / de,vua |
| qa | `qa-opus`: claude / claude-opus-5-5 / kho · `qa-sonnet`: claude / claude-sonnet-5-5 / vua · `qa-flash`: agy / gemini-3.8-flash-medium / de,vua |
| bao-mat | `sec-opus`: claude / claude-opus-5-5 / de,vua,kho |

Thêm test: file mẫu phải qua `load_from` không lỗi, với config có engine `agy`.

### 7. Test (`agent-brain/tests/departments.rs` mới và unit test trong module)
- Validate: khóa phòng lạ, mã trùng giữa hai phòng, engine `người:x`, engine dsh, level rỗng hoặc lạ, model có ký tự cấm → lỗi đúng trường. File không có → rỗng.
- `pick`:
  - đúng mức
  - không ai đúng mức → lấy cả phòng
  - engine không sẵn sàng bị bỏ
  - giữ người cũ cùng vai
  - review ưu tiên khác họ model (dev `claude-sonnet-5-5` → qa không chọn `tc-sonnet` mà chọn `qa-opus` / `qa-flash`)
  - ít run `running` thắng
  - điểm TB chỉ tính khi đủ 5 lượt
  - vai không thuộc phòng nào → `None`
- `resolve_for_run` + `plan_for` với đội hình `phong-ban`: argv claude có `--model claude-opus-5-5`; argv agy có `--model gemini-3.8-flash-high`; `runs.member_id` được ghi. Đội hình `mac-dinh` không đổi hành vi (test hiện có vẫn qua).
- Triage: thiếu `level` → `vua`; `level: "x"` → lỗi.
- Dùng fake engine và config tạm như các test `engine_routing.rs` / `task_team.rs` hiện có. Không chạy claude hay agy thật. Biến `HOME` / `AGENT_BRAIN_*` trỏ vào thư mục tạm.

## Không sửa
`Cargo.toml` (nếu bắt buộc thêm crate thì dừng lại và ghi vào tóm tắt), `~/.config/agent-brain/*` và `~/.local/share/agent-brain/*` thật, dashboard thật cổng 7437, repo dự án ngoài, `.planning/STATE.md`, `ROADMAP.md`, file `.planning/` khác, `~/.claude-orca`. Không làm phần giao diện dashboard (thuộc plan `departments-dashboard`).

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # base 153, không được tăng
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
spikes/lib/scan-secrets.sh
```
Chạy thử `dept check` và `dept list` với `HOME` tạm chứa bản sao file mẫu: kết quả phải thoát 0.

## Commit
Hai commit:
1. `feat(agent-brain): departments, members and task level`: mục 1, 2, 4, 6 và test tương ứng.
2. `feat(agent-brain): pick a department member per role and run on its model`: mục 3, 5 và test tương ứng.

`git add` đúng file, không `-A`, không `--no-verify`, không push. Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Tóm tắt gồm: file sửa, commit, chỗ lệch plan, clippy base/sau, kết quả kiểm tra.
