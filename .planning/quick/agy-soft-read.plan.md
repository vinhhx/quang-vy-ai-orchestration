# Plan: agy Dev — đọc/liệt kê ngoài danh sách trong worktree không làm dừng run

Repo: `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`.

## Bối cảnh (lỗi thật, task e2e 68 ngày 2026-11-14)

agy không có hook chặn từng công cụ như Claude. `roles/mod.rs::Observer::check_agy_tool` đọc luồng stream: gặp một bước bị `guard::agy_tool_decision` từ chối thì lưu `violation` và dừng run. Autopilot dừng task với lý do `vi_pham_giao_thuc`. Gemini Flash làm Dev hay "nhìn quanh" trước khi code, nên 4/5 task con bị dừng ở lượt Dev đầu tiên:
- run 380: `read_file tsconfig.json` (file không có trong danh sách đọc)
- run 389: `read_file tsconfig.json` (file KHÔNG tồn tại; mục khai nằm sai trong `read_dirs`)
- run 392: `run_command ls -la .gsd/projects/_template` (lệnh không có trong "Lệnh được chạy")
- run 381: `write_file .gsd/.env.cnc-tech` (ghi `.env*`: phải tiếp tục DỪNG)

Mục tiêu: thao tác **chỉ đọc** bên trong gốc dự án (worktree) hoặc trong dự án nguồn chỉ đọc, trên đường dẫn không bị cấm, thì **không** tính vi phạm. Chỉ ghi cảnh báo. Mọi thao tác ghi, chạy lệnh không chỉ đọc, đọc đường dẫn cấm (`.env*`, `.git`, `.planning`, `report::sensitive_path`) hoặc đọc ngoài dự án thì giữ nguyên hành vi cũ (dừng run).

## Việc 1: `guard::agy_soft_read`

File: `agent-brain/src/guard.rs`.

Thêm `pub fn agy_soft_read(policy: &Policy, tool_name: &str, params: &serde_json::Map<String, serde_json::Value>) -> Option<String>`. Hàm trả `Some(mô tả ngắn)` khi bước là đọc an toàn, ngược lại trả `None`. Dùng `agy_tool_payload(tool_name, params, &policy.root)` để quy về tên công cụ kiểu Claude (như `agy_tool_decision_inner` đang làm):

1. `Read`, `Glob`, `Grep` (và mọi công cụ agy quy về chúng: `read_file`, `view_file`, `list_dir`, `find_by_name`, `grep_search`…). Lấy đường dẫn (`file_path` / `path`; rỗng thì là gốc), chuẩn hoá giống `agy_listing_decision` (`nfc`, nối với root nếu tương đối, `realpath_nfc`). Đọc an toàn khi:
   - đường dẫn nằm trong `realpath(policy.root)` **hoặc** trong một `policy.source_roots`;
   - và `grant_denied_rel(rel)` là false (rel tính theo root tương ứng).

   File không tồn tại vẫn được tính là an toàn. Symlink phải resolve trước khi so: symlink trỏ ra ngoài root thì KHÔNG an toàn, ví dụ `.gsd/.env.cnc-tech` → repo chính.
2. `Bash` / `run_command`: an toàn chỉ khi thoả TẤT CẢ:
   - cwd (`AGY_CWD_KEY`) rỗng hoặc resolve bằng root;
   - lệnh không chứa ký tự/chuỗi nào trong `; | & > < $( \` \n` và không có biến môi trường đứng trước;
   - từ đầu tiên thuộc `ls, cat, head, tail, wc, stat, file, tree, pwd, grep, rg, find`;
   - `find` không có `-exec`, `-execdir`, `-delete`, `-ok`, `-okdir`, `-fprint`, `-fprintf`, `-fls`;
   - `grep`/`rg` không có `-r`… thì vẫn được (chỉ đọc);
   - mọi đối số không bắt đầu bằng `-`, khi coi là đường dẫn (tương đối với root), phải thoả điều kiện của mục 1. Đối số là pattern của grep/rg (đối số không-cờ đầu tiên sau `grep`/`rg`) thì bỏ qua kiểm đường dẫn.

   Tách từ bằng `shell_words` hoặc `split_whitespace` (crate đã dùng gì thì dùng nấy). Đối số có dấu nháy không tách được thì coi là KHÔNG an toàn.
3. Mọi công cụ khác (ghi, sửa, `browser_*`…) → `None`.

Unit test (`#[cfg(test)]` trong `guard.rs`, dùng tempdir làm root, tạo file `a.txt`, thư mục `sub/`, `.env.x`, symlink `link.env` → file ngoài root):
- `read_file a.txt` → Some.
- `read_file khong-ton-tai.json` → Some.
- `read_file .env.x` → None.
- `read_file /etc/passwd` → None.
- `read_file link.env` (symlink ra ngoài) → None.
- `list_dir sub` → Some.
- `run_command "ls -la sub"` → Some.
- `run_command "cat .env.x"` → None.
- `run_command "ls; rm -rf x"` → None.
- `run_command "find . -delete"` → None.
- `run_command "grep -rn foo sub"` → Some.
- `run_command "rm a.txt"` → None.
- `write_file a.txt` → None.

Tên tham số agy: xem `agy_tool_payload` và các test sẵn có của `agy_tool_decision` trong `guard.rs`. Dùng đúng khoá mà các test đó dùng (ví dụ `"CommandLine"`, `"Path"`, `"AbsolutePath"`, `"DirectoryPath"`…).

## Việc 2: Observer không dừng run vì đọc an toàn

File: `agent-brain/src/roles/mod.rs` (`Observer::check_agy_tool`).

Khi `decision.allow == false`, trước khi lưu `violation`, gọi `crate::guard::agy_soft_read(policy, &tool, &params_clone)`. Cần giữ một bản `params` trước khi move vào `Value::Object`. Nếu `Some(desc)`:
- `crate::term::emit(&format!("agy Dev đọc ngoài danh sách của plan (cho qua, không dừng): {tool} {shown} — {}", decision.reason))`;
- đẩy một dòng tương tự (≤200 ký tự) vào `self.source_warnings`, hoặc một vec mới `soft_reads` nếu `source_warnings` có nghĩa riêng; đưa vec đó vào `metrics_json` (key `agy_soft_reads`, tối đa 50 mục) ở chỗ metrics được ghi;
- `return` mà không set `violation`/`stop_reason`.

Không đổi gì với `check_agy_init` và đường đi của engine Claude.

Test: một test cho Observer nếu có test harness sẵn (tìm test hiện có của `check_agy_tool`). Không có thì đủ với test ở Việc 1, ghi rõ trong summary.

## Ngoài phạm vi

Không đổi `decide`, `agy_tool_decision`, validator plan, `sensitive_path`, engine Claude. Không đụng `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` chưa track, `~/.config/agent-brain`, `~/.local/share/agent-brain`, `~/.local/bin/agent-brain`.

## Verify

```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo build --release --offline --locked 2>&1 | tail -3
cargo test --offline --locked 2>&1 | grep -E "^test result|FAILED|panicked" | tail -20
bash ../spikes/lib/scan-secrets.sh
```

Đạt khi: build OK, `cargo test` 0 failed, có ≥ 12 test mới của `agy_soft_read` và đều pass.

## Commit và summary

Mỗi việc một commit (`git add <file cụ thể>`, không `-A`, không `--no-verify`):
- `guard: agy_soft_read cho đọc an toàn trong worktree`
- `roles: agy Dev đọc ngoài danh sách chỉ cảnh báo, không dừng run`

Viết `.planning/quick/agy-soft-read.summary.md` (file đổi, commit, khoá tham số agy đã dùng, lệch plan) và commit.
