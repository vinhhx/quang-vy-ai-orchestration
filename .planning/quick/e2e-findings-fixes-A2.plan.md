# Plan 3-A2: sửa lỗi review phase A (2026-11-08)

Tiếp theo `.planning/quick/e2e-findings-fixes.plan.md`. Đọc mục 0 (không hard code) và Phase A của plan đó trước.

Code hiện tại là commit phase A `994cd2b`. Chỉ sửa trong:
- `agent-brain/src/**`
- `agent-brain/tests/**`
- `agent-brain/assets/**`

## Thay đổi

1. **Kiểm ngân sách lúc duyệt dùng sai bảng giá** (`src/autopilot/claim.rs` `approve_check`, khoảng `:388`).
   - Hiện tại: gọi `crate::config::default_pricing()`. Autopilot thì dùng `cfg.pricing` (`autopilot/mod.rs:443`, `api_tasks.rs:1437`), nên lúc duyệt và lúc autopilot có thể kết luận khác nhau về ngân sách.
   - Sửa: thêm tham số `pricing: &BTreeMap<String, Price>` vào `approve_check`, và truyền `&ctx.cfg.pricing` (`autopilot/mod.rs:800`) và `&st.cfg.pricing` (`api_tasks.rs:3115`).
   - Cập nhật test `claim.rs:558`.
   - Logic tính ngân sách đang bị chép lại ở `approve_check` và `taskstate` (nếu có). Gom về một hàm dùng chung, ví dụ `pricing::budget_exhausted_reason(runs, pricing, wf, aj, now) -> Option<String>`. Hàm trả câu "Hết ngân sách quy trình (…)". Gọi hàm này ở cả `approve_check` lẫn `taskstate::build_approve`.
2. **Chuỗi sau task `qa_pass` bị thêm tài liệu rỗng** (`src/orchestrator/scheduler.rs`, nhánh `is_success` khi status khác `answered`).
   - Hiện tại: `prior_answer = Some((id, String::new()))`. Sau đó `add_to_proposal` được gọi với nội dung rỗng, có thể làm đề xuất thất bại.
   - Sửa: chỉ chuyển tài liệu khi có câu trả lời thật (status `answered`). Task thành công khác thì đi tiếp mà không thêm tài liệu.
3. **Số file trong thông báo vi phạm** (`roles/dev.rs`, `roles/lead.rs`, `roles/qa.rs`, chỗ gọi `record_violation`).
   - Hiện tại: câu thông báo đếm danh sách đã cắt 50.
   - Sửa: dùng tổng số file thật. Khi bị cắt thì thêm "… (và K file khác)". `details.files` vẫn giữ tối đa 50, và thêm `details.total`.
4. **Thêm các test còn thiếu.** Phase A chỉ có 6 test; plan yêu cầu thêm:
   - `approve_check`: plan còn dòng vướng mắc chưa `[x]` ⇒ Err chứa "vướng mắc chưa đánh dấu [x]"; toàn bộ đã `[x]` ⇒ không lỗi vì lý do này.
   - `approve_check`: run đã dùng hết `budget_minutes` ⇒ Err "Hết ngân sách"; sau khi `auto_json.budget_extra_minutes` đủ lớn ⇒ không lỗi vì lý do này. Dùng `cfg.pricing` của test (giá tùy chỉnh) để chứng minh mục 1.
   - API `POST /api/tasks/{id}/auto/extend-budget`: vượt `max_budget_extension_minutes` ⇒ 4xx; hợp lệ ⇒ `auto_json` được cộng thêm. Viết theo mẫu test dashboard có sẵn trong `tests/`.
   - `sse::status_json` và JSON runs của task có `error` và `warnings`.
   - Render Telegram (`render.rs`) của RunFailed khi `runs.error` có giá trị ⇒ có trích lỗi, không còn "xem log của run này". Chuỗi dài hơn 500 ký tự ⇒ bị cắt.
   - Scheduler:
     - task trước `auto_state='ended'` + `qa_fix` ⇒ đề xuất `failed`;
     - task trước `answered` ⇒ đề xuất chạy, có tài liệu `phan-tich-<id>.md`;
     - task trước `qa_pass` ⇒ đề xuất chạy, không có tài liệu (mục 2);
     - run gần nhất `timed_out` nhưng task `stopped` ⇒ đề xuất vẫn chờ.

     Có thể đã có test tương tự; kiểm trước để không trùng.
   - Vi phạm Dev sai nhánh và Dev có file ngoài plan ⇒ `runs.error` khác NULL và `metrics_json.violation.kind` đúng. Dùng fake engine sẵn có trong `tests/` (xem các test dev hiện tại).

## Không sửa
- `Cargo.toml`.
- `~/.config/agent-brain/*`, `~/.local/share/agent-brain/*`.
- Dashboard thật cổng 7437.
- Repo ngoài.
- `.planning/STATE.md`, `ROADMAP.md`, các file `.planning/` khác, `~/.claude-orca`.

## Cách kiểm tra
```bash
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain
cargo test --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'   # > số sau phase A, failed 0
cargo clippy --all-targets 2>&1 | grep -c "^warning\|^error"     # ≤ 152
grep -n "default_pricing" src/autopilot/claim.rs                 # chỉ còn trong test
cd /mnt/data/IDE/QuangVy-Orchestration
node scripts/dashboard-js-check.mjs
bash scripts/dashboard-ui-lint.sh
bash scripts/build-css.sh --check
bash spikes/lib/scan-secrets.sh
```

## Commit
Một commit: `fix(agent-brain): approve budget check uses configured pricing, chain after qa_pass, phase A tests`.
- `git add` đúng file, không `-A`, không `--no-verify`, không push.
- Kết thúc message bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

Tóm tắt gồm: file sửa, commit, chỗ lệch plan, clippy trước/sau, kết quả kiểm tra.
