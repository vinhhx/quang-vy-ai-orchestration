# Plan: trường `claude_config_dir` cho engine `anthropic` (engine gắn tài khoản, không theo profile dự án)

Repo: `/mnt/data/IDE/QuangVy-Orchestration` (crate `agent-brain/`), nhánh `main`. Đọc toàn bộ file này trước khi làm.

## Mục tiêu (quyết định của người dùng, 2026-10-29)
Người dùng muốn chọn tài khoản Claude theo **engine**, không theo profile của dự án, và dựng đội hình bằng cách chọn engine cho từng vai:
- `claude` (có sẵn) → tài khoản `vinhhoangxuan@bizfly.vn` = thư mục `~/.claude` (giữ nguyên hành vi hiện tại: chạy theo profile dự án; profile work = `~/.claude`).
- `claude-max` → tài khoản `hieutranvan@vccorp.vn` = thư mục `~/.claude-max`, chạy được trên mọi profile (work và personal).
Quota là theo tài khoản; run ghi `runs.engine` = id engine nên quota đã tách theo engine. Không đổi cơ chế quota.

Đảo ngược một phần D-36 ("claude-max chỉ cho personal"): engine có `claude_config_dir` KHÔNG bị khóa profile (trừ khi người dùng tự ghi `allowed_profiles`).

## Bối cảnh code (đã đọc)
- `agent-brain/src/profile.rs::build_env` đặt `CLAUDE_CONFIG_DIR` từ `profiles.<p>.claude_config_dir` (có mở rộng `~`). Env này đi vào `engine.rs::anthropic_plan(.., base_env)`.
- `agent-brain/src/config.rs`: `AnthropicDef`, `ANTHROPIC_ENGINE_KEYS` (~dòng 488), `parse_anthropic_entry` (~dòng 665), `effective_allowed_profiles` (~dòng 187, `pin_profile` suy ra `allowed_profiles`).
- `engine.rs::anthropic_plan` (~dòng 614): `let mut env = base_env; let native = a.base_url.is_none();`
- `engine.rs::profile_refusal` (~dòng 561) chặn theo `effective_allowed_profiles`.
- `engine.rs` ~dòng 320-340: `engine_status` dựng `EngineStatus` (readiness `SanSang/ThieuKey/ThieuLenh/ChuaKiem`).
- `claude` có sẵn là id dành riêng (`ENGINE_CLAUDE`), không khai báo trong `engines` được. Không đổi điều này.

## Thay đổi
| # | File | Dòng / hàm | Sửa gì | Lý do |
|---|---|---|---|---|
| 1 | `agent-brain/src/config.rs` | `AnthropicDef` + `ANTHROPIC_ENGINE_KEYS` (đổi kích thước mảng 7→8) + chỗ khởi tạo `AnthropicDef` ở ~dòng 216 | Thêm trường `pub claude_config_dir: Option<String>` và khóa `"claude_config_dir"`; khởi tạo `None` ở mọi chỗ dựng `AnthropicDef` (kể cả mặc định/test helper). | Cấu hình gắn tài khoản theo engine |
| 2 | `agent-brain/src/config.rs` | `parse_anthropic_entry` | Parse `claude_config_dir`: phải là chuỗi một dòng, không rỗng, dài ≤ 200, bắt đầu bằng `~/` hoặc `/`, không chứa `..`, không chứa ký tự điều khiển. Lỗi: `Cấu hình engines.<id>.claude_config_dir không hợp lệ (đường dẫn tuyệt đối hoặc bắt đầu bằng ~/, không có ..)`. Từ chối khi có `base_url` (`.claude_config_dir chỉ dùng cho engine không có base_url`) và khi có cùng `pin_profile` (`.claude_config_dir không dùng cùng pin_profile`). Cho phép cùng `allowed_profiles`. | Chặn cấu hình mơ hồ |
| 3 | `agent-brain/src/profile.rs` | cạnh `build_env` | Tách phần mở rộng `~` thành `pub fn expand_home(dir: &str, env: &BTreeMap<String,String>) -> String` (giữ nguyên logic `HOME` và `posix_normpath` hiện có) và cho `build_env` gọi lại hàm này. Hành vi của `build_env` không đổi. | Dùng chung, không nhân đôi logic |
| 4 | `agent-brain/src/engine.rs` | `anthropic_plan`, ngay sau `let mut env = base_env;` | Nếu `a.claude_config_dir` là `Some(dir)` (chỉ xảy ra khi không có `base_url`): `env.insert("CLAUDE_CONFIG_DIR", profile::expand_home(dir, &env))`. Không đổi gì khác (`unset_env`, `strip_key_envs`, guard hook giữ nguyên). `native` vẫn `true`. | Ghi đè tài khoản của profile bằng tài khoản của engine |
| 5 | `agent-brain/src/config.rs` | `effective_allowed_profiles` | Không đổi logic: engine có `claude_config_dir` mà không có `pin_profile`/`allowed_profiles` trả `None` = chạy mọi profile. Thêm comment một dòng nói rõ điều này. | Đảo D-36 có chủ đích |
| 6 | `agent-brain/src/engine.rs` | `engine_status`, nhánh `EngineSpec::Anthropic` | Nếu có `claude_config_dir` và `readiness == SanSang` và thư mục (sau `expand_home` với `HOME` thật) không tồn tại hoặc không có file `.claude.json` trong đó: `set_readiness(&mut st, Readiness::ThieuLenh, Some("Thư mục config <dir> chưa có hoặc chưa đăng nhập Claude (thiếu .claude.json) — chạy: CLAUDE_CONFIG_DIR=<dir> claude rồi /login."))`. Không thêm giá trị readiness mới (theo D-44). Hàm phải kiểm được trong test: nhận `home`/`path` qua tham số hiện có hoặc một closure kiểm file như `probe`; theo mẫu sẵn có của hàm. | Báo sớm khi tài khoản chưa đăng nhập |
| 7 | `agent-brain/tests/engine_routing.rs` (hoặc `engine_guard.rs`, chọn file có sẵn fake claude ghi env) và unit test trong `config.rs`, `engine.rs` | cuối module test | Test: (a) `claude_config_dir` hợp lệ được parse; các trường hợp lỗi ở dòng 2 (`..`, tương đối, rỗng, cùng `base_url`, cùng `pin_profile`); (b) `anthropic_plan` cho entry có `claude_config_dir: "~/.claude-max"` đặt `CLAUDE_CONFIG_DIR` = `$HOME/.claude-max` dù `base_env` đang có `CLAUDE_CONFIG_DIR` của profile khác; entry không có trường này giữ `CLAUDE_CONFIG_DIR` của `base_env`; (c) một task trên dự án profile **work** với engine có `claude_config_dir` và không `allowed_profiles` KHÔNG bị `profile_refusal`; (d) cùng entry nhưng có `allowed_profiles: ["personal"]` vẫn bị chặn trên dự án work; (e) readiness `thieu_lenh` khi thư mục không tồn tại. Test hiện có cho `pin_profile` phải giữ nguyên và pass. | Chốt hành vi |
| 8 | `docs/config.engines.example.json` | entry `claude-max` | Đổi thành: `"id": "claude-max"`, `"kind": "anthropic"`, `"label": "Claude Max (hieutranvan@vccorp.vn)"`, `"command": "claude"`, `"claude_config_dir": "~/.claude-max"`, bỏ `pin_profile`, đổi `_note` thành: `Gắn tài khoản theo engine, chạy trên mọi profile. QA luôn là claude có sẵn (D-39).` Giữ nguyên `models`. | Ví dụ khớp thiết kế mới |
| 9 | `agent-brain/README.md` | bảng khóa engine (~dòng 339) và đoạn `pin_profile và allowed_profiles` (~dòng 351) | Thêm dòng bảng: `claude_config_dir` / `anthropic` không có `base_url` / "Thư mục config Claude dùng cho engine này (ví dụ `~/.claude-max`): engine luôn chạy bằng tài khoản đăng nhập trong thư mục đó, bất kể profile dự án; ghi đè `claude_config_dir` của profile. Không dùng cùng `pin_profile` hay `base_url`. Quota tính theo tài khoản của thư mục này." Thêm 2-3 câu vào đoạn `pin_profile`: khác biệt giữa `pin_profile` (khóa engine vào profile) và `claude_config_dir` (chọn tài khoản, không khóa profile); chạy work project qua tài khoản khác là lựa chọn của người dùng. Thêm dòng xử lý sự cố cho thông báo readiness mới. | Tài liệu |
| 10 | `.planning/phases/06-multi-engine-media-branch/06-CONTEXT.md` | sau dòng D-44 (~dòng 91) | Thêm một dòng `- **D-45 (revises D-36, user 2026-10-29):** An engine may carry \`claude_config_dir\` ... ` nêu: engine chọn tài khoản qua thư mục config, không bị khóa profile; D-36 chỉ còn áp dụng cho engine có `pin_profile`; lý do: người dùng dựng đội hình theo mục đích và chia quota theo tài khoản. | Ghi lại quyết định |

## Không được đụng tới
- `config.json` của người dùng (`~/.config/agent-brain/*`) — người dùng tự cập nhật sau khi build.
- `.planning/STATE.md`, `ROADMAP.md`, mọi file `.planning/` khác, `.claude/worktrees/`, `~/.claude-orca`, `~/.claude-max`, `~/.claude`.
- Logic quota (`quota.rs`), `profile::build_env` về hành vi (chỉ refactor tách hàm), `guard.rs`, engine `agy` và `dsh`, `profile_refusal` (không đổi code; hành vi mới đến từ `effective_allowed_profiles` trả `None`).
- Không đọc, in hay ghi nội dung `.claude.json` (có token) — chỉ kiểm file tồn tại.
- `git add -A`, `--no-verify`: cấm. Commit theo đường dẫn.

## Cách kiểm tra
Trong `/mnt/data/IDE/QuangVy-Orchestration/agent-brain`:
```bash
cargo build --offline --locked 2>&1 | tail -10
cargo test --lib 2>&1 | grep -E "^test result|FAILED"
cargo test --test engine_routing --test engine_guard --test engine_agy 2>&1 | grep -E "^test result|FAILED"
cargo test --test dashboard_stats --test dashboard_settings 2>&1 | grep -E "^test result|FAILED"
cargo clippy --all-targets 2>&1 | grep -E "src/(config|engine|profile)\.rs" | head   # không cảnh báo mới ở dòng bạn thêm
```
Acceptance:
1. Config có `{"id":"claude-max","kind":"anthropic","claude_config_dir":"~/.claude-max","models":{...}}` được nạp; run của engine này đặt `CLAUDE_CONFIG_DIR=$HOME/.claude-max` trên cả dự án work lẫn personal.
2. Entry `pin_profile` cũ vẫn bị khóa profile như trước; mọi test cũ pass.
3. `claude_config_dir` + `base_url` hoặc + `pin_profile`, đường dẫn tương đối hoặc có `..` đều bị từ chối bằng thông báo tiếng Việt.
4. Thư mục thiếu `.claude.json` → engine hiện `thiếu lệnh` kèm lý do, không panic, không đọc nội dung file.
5. Chỉ các file trong bảng Thay đổi bị sửa.

## Quy trình
- Branch: `main` hiện tại. Commit theo nhóm: `feat(agent-brain): engine claude_config_dir` (code + test), `docs: claude_config_dir in README, example and D-45`.
- Mỗi commit kèm `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Tóm tắt cuối: file đã sửa, commit, chỗ lệch plan, kết quả từng lệnh verify.
