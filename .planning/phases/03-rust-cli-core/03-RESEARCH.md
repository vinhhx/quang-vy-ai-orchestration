# Phase 3: Rust CLI Core - Research

**Researched:** 2026-10-16
**Domain:** Rust CLI that supervises a headless `claude -p` child (process groups, signals, stream-json capture, SQLite, git fast-path, static musl binary), porting the Phase 2 Python `brain` protocol.
**Confidence:** HIGH on stack, process control, build/dist (all prototyped and run on this machine); MEDIUM on live `claude` behaviour (never run `claude` in research, by instruction); LOW only where tagged `[ASSUMED]`.

> Provenance tags used below: `[VERIFIED: ...]` = observed this session (file read with path:lines and a verbatim quote, or a command run, output shown); `[CITED: ...]` = taken from a document in the repo/docs; `[ASSUMED]` = not verified, needs confirmation. Prototype crates used for every "ran it" claim live in the research scratchpad only (nothing was added to the repo, `~/.cargo`, `~/.rustup` or the system).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Cách xây & bàn giao từ Phase 2
- **D-01:** Code Rust do **GSD execute-phase** viết (plan + executor, test theo từng module, commit nguyên tử) — **không** dogfood qua `brain lead/dev/qa` như spec §8 gợi ý. Giao thức Phase 2 tiếp tục được kiểm chứng trên các dự án khác của người dùng.
- **D-02:** Script Python `brain` (Phase 2) **giữ đến khi `agent-brain` đạt parity** (qua test fixture chung + 1 task thật chạy bằng binary), sau đó **bỏ symlink khỏi PATH**; code `brain/` giữ lại trong repo làm bản tham chiếu, không bảo trì song song.
- **D-03:** Parity (CLI-02, CLI-03) chứng minh bằng **bộ test case trung lập ngôn ngữ**: các ca định tuyến/quét/parse `.plan.md` (P1-01..P1-05, P1-10 và tương tự) đặt thành file JSON (input → kết quả mong đợi) trong repo; cả test Python lẫn test Rust cùng đọc một bộ file này, lệch hành vi là test đỏ. Phase 2 nên tạo bộ fixture này (hoặc Phase 3 trích ra từ test Phase 2 làm việc đầu tiên). — **Reversibility:** costly — định dạng fixture là hợp đồng giữa hai bộ test; đổi format phải sửa cả hai phía.
- **D-04:** DIST-01: build **static musl `x86_64-unknown-linux-musl`** (rusqlite `bundled`), chỉ Linux x86_64 (khớp Phase 1 D-08). aarch64, macOS, Windows ngoài phạm vi.

### Nơi lưu dữ liệu & danh tính task
- **D-05:** **`~/.config/agent-brain/projects_map.json` là nguồn sự thật** cho dự án (người dùng vẫn sửa tay alias/`name_vi`/`profile` như Phase 2; `agent-brain projects scan` ghi file này với cùng luật không ghi đè trường sửa tay + cảnh báo alias trùng). Bảng SQLite `projects` chỉ là **bản phản chiếu**, đồng bộ từ JSON mỗi lần chạy, để `tasks`/`runs` tham chiếu. — **Reversibility:** costly — đảo chiều (SQLite làm chủ) cần migrate và bỏ thói quen sửa tay.
- **D-06:** CSDL SQLite ở **`~/.local/share/agent-brain/brain.db`** (XDG; config ở `~/.config/agent-brain/`). Một DB chung cho mọi dự án để `stats` gộp được. Có biến môi trường ghi đè đường dẫn (cho test). Tên biến cụ thể: Claude quyết.
- **D-07:** **Task = nhánh `task/<slug>`.** `run lead` tạo task (dòng `tasks` + nhánh `task/<slug>` từ HEAD, giữ luật Phase 2 D-14: working tree bẩn thì dừng). `run dev`/`run qa` gắn vào task có nhánh đang checkout trong dự án đó (không ở nhánh task → báo lỗi, không đoán). Mỗi lần chạy một vai trò = một dòng `runs` thuộc task. Commit base ghi trong `.plan.md` (Phase 2 D-14) dùng làm gốc diff.
- **D-08:** `task_id` là **số nguyên ngắn** (rowid SQLite). `run` in ra ngay khi bắt đầu, ví dụ `task 42, run 7`. `agent-brain kill <task_id>` gửi tín hiệu tới process group của run đang chạy của task đó (PGID lưu trong `runs.pid` hoặc cột tương đương).

### Hiển thị khi chạy & kill
- **D-09:** Terminal mặc định: **mỗi sự kiện stream-json một dòng gọn** (giờ, loại, tóm tắt — ví dụ `12:03:41 tool Read src/main.rs`, `assistant: <~100 ký tự đầu>`), cuối run in **một dòng tổng kết** (số turn, token, thời gian, trạng thái). `--verbose` in đầy đủ text assistant và kết quả tool. Toàn bộ log vẫn vào bảng `events` kèm `raw_json`.
- **D-10:** **Ctrl+C lần 1** khi đang `run`: SIGTERM cả process group → chờ thời gian ân hạn → SIGKILL cả group; run ghi trạng thái `killed` kèm token đã nhận được. **Ctrl+C lần 2** trong lúc chờ: SIGKILL ngay. Đóng terminal (SIGHUP) xử lý như Ctrl+C, không hỏi xác nhận.
- **D-11:** Thời gian ân hạn SIGTERM → SIGKILL **mặc định 3 giây, cấu hình được** (ví dụ `kill_grace_secs` trong file cấu hình) — đủ biên cho P2-02 (kiểm tra sau 5 giây). Có thể chỉnh theo kết quả spike S4.
- **D-12:** **Đối soát khi khởi động:** mỗi lần `agent-brain` chạy, các dòng `runs` đang `running` mà process group không còn tồn tại → đánh dấu `lost` (ended_at = lúc phát hiện). `kill` trên task không còn tiến trình thì báo rõ, **không bao giờ gửi tín hiệu tới PID có thể đã bị tái sử dụng** cho tiến trình khác.

### Fast-path & thống kê
- **D-13 (`status`, cấp 1, 0 token):** in nhánh hiện tại, `git status --short`, `git log --oneline -5`, `git diff --stat` so với commit base của task (nếu đang ở nhánh task), và run gần nhất của task kèm kết quả từ SQLite. Không gọi AI, không ghi dòng `runs`.
- **D-14 (`inspect`, cấp 2):** prompt cố định "review ngắn": tóm tắt thay đổi, rủi ro dễ thấy, có khớp `.plan.md` không (nếu có). Diff so với commit base của task, hoặc HEAD nếu không có task. Model **Sonnet**, `--max-turns 1`. Kết quả chỉ in ra terminal; ghi một dòng `runs` với `role = inspect` (để `stats` đếm token). Diff quá lớn thì cắt và báo rõ đã cắt.
- **D-15 (`inspect` không lưu session — P2-07):** ưu tiên cờ CLI không lưu session nếu có (researcher kiểm chứng tên/hành vi cờ); nếu không có hoặc không hoạt động thì **xóa file session theo `session_id`** trong config dir của profile sau khi run xong. P2-07 phải có test.
- **D-16 (`stats`):** tổng (số run, input/output/cache read/cache write token, `cost_usd`) + bảng chia theo **dự án** và theo **vai trò**; `--profile work|personal` lọc (STAT-01); `--since <khoảng>` (ví dụ `7d`) chọn cửa sổ thời gian. `cost_usd` hiển thị nhãn **"tương đương"** vì gói subscription không tính tiền theo token.

### Claude's Discretion
- Tên crate/module chi tiết (bám cấu trúc spec §4.3), định dạng file cấu hình (TOML/JSON), tên biến môi trường ghi đè đường dẫn DB.
- Schema SQLite chi tiết ngoài các cột spec §4.2 (thêm cột cho PGID, trạng thái `killed`/`lost`, base commit, `role = inspect`), cách migrate schema.
- Cách phân loại sự kiện stream-json và rút gọn mỗi dòng hiển thị (dựa trên fixture thật từ `spikes/s1/raw/` nếu đã có).
- Cách đọc 3 prompt vai trò của Phase 2 (nhúng vào binary bằng `include_str!` hay đọc file) — miễn hành vi giống Phase 2 (CLI-03).
- Mã thoát (exit code) cho từng lỗi, miễn Fallback Ask giữ "thoát mã khác 0" như Phase 2 D-09.

### Deferred Ideas (OUT OF SCOPE)
- `agent-brain status` không truyền dự án → tổng quan một dòng cho mọi dự án (nhánh, bẩn/sạch, task mở) — tính năng mới, cân nhắc ở Phase 5 (dashboard) hoặc sau.
- Build aarch64 / macOS / Windows (Job Object) — ngoài phạm vi; xem Phase 1 D-08.
- Warm reuse `--resume` (FAST-03) — vẫn ở v2, quyết định ở `/gsd-discuss-phase 4`.

**Stale-context note (from the orchestrator, confirmed):** CONTEXT.md says `brain/`, `spikes/` "chưa tồn tại". Both phases are complete and their artifacts exist; this research read them and they are the executable specification to port.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `agent-brain run lead "<dự án>" "<yêu cầu>"` runs on a real project, log shows each step | Runner + stream parser + terminal sink (Architecture Patterns 1, 2, 5); fixtures `spikes/s1/raw/s1.jsonl`; fake claude strategy |
| CLI-02 | `projects scan` + accent-insensitive match + Fallback Ask == Phase 2 behaviour | Contract fixtures `tests/contract/{normalize,routing,scan}.json` (prototype: normalize 8/8 pass in Rust) |
| CLI-03 | `run lead\|dev\|qa` with the 3 Phase 2 prompts, protocol ROLE-01..04, 06 kept | Port map of `brain/role_*.py`, `planfile.py`, `guard.py`; `include_str!` of prompts/templates; fixtures `plan.json`, `qa_report.json`; new `guard.json` |
| PROF-02 | Pick `CLAUDE_CONFIG_DIR`/env by project `profile` (`profile.rs`) | `tests/contract/profile_env.json`; `build_env` port with injected base env |
| PROC-01 | `kill <task_id>` during `sleep 300`: no `claude`/`sleep` left after 5 s | Verified kill algorithm (prototype ran 5 scenarios, 0 survivors); S4 facts; PID-reuse guard |
| PROC-02 | Terminal close / Ctrl+C leaves no process behind | Signal design (SIGINT x2, SIGTERM, SIGHUP); prototype verified; pitfalls (EPIPE/EIO on closed terminal) |
| LOG-01 | `runs` has all token fields, cost, session_id, pid, status, times; events with `raw_json` | DDL proposal, result-event field map from the real fixture, partial-usage fallback |
| LOG-02 | Bad JSON line stored raw, program continues, unknown fields ignored | Byte-level line reader (`read_until` + lossy UTF-8), `serde_json::Value` soft parse, bad-line kind |
| FAST-01 | `status` = git only, 0 token | `fastpath.rs` design; test with `claude_bin` pointing at a missing path |
| FAST-02 | `inspect` = exactly 1 turn, no saved session | `--no-session-persistence` + `--tools ""` + `--max-turns 1` (S8 variant B); E2BIG cap; session-file cleanup fallback with path rules |
| STAT-01 | `stats --profile work` counts only work runs | `runs.profile` snapshot column + SQL; `--json` for tests |
| DIST-01 | Copied binary runs with nothing installed | Static musl build recipe (3 verified routes), `strip`/`lto`, automated static check |
</phase_requirements>

## Summary

Phase 3 is a faithful port plus three genuinely new subsystems. The port part (routing, scan/add, `.plan.md` parse/validate, QA verdict, profile env, role tool allowlists, Dev guard hook) is fully specified by Phase 2 code and by six language-neutral fixture files that already exist (`tests/contract/*.json`, D-03 is already satisfied). The new parts are (1) a process supervisor that must kill a tree whose Bash children live in *different sessions* from `claude`, (2) a tolerant stream-json reader feeding SQLite, and (3) a fully static binary. All three were prototyped and run on this machine: a musl static binary with bundled SQLite builds and runs, a tokio-based supervisor killed a fake `claude` tree cleanly in five scenarios (cooperative SIGTERM, SIGTERM-ignored escalation, double Ctrl+C, SIGHUP, SIGTERM-to-supervisor) leaving zero survivors, and the Vietnamese `normalize`/`slugify` port matches all 8 cases of `normalize.json` with `unicode-normalization` + `regex`.

Four findings change the plan versus the written spec: (a) spec §3.2's "kill the process group" is **not sufficient** on its own — S4 recorded that claude's Bash child and its `sleep` sit in their own session and process group, so `killpg(claude)` never touches them and a SIGKILLed claude orphans them; the kill routine must record descendants from `/proc` *before* signalling and sweep survivors by pid+start-time (verified in the prototype). (b) The Dev guard hook was a Python script (`python -I brain/guard.py`); to satisfy DIST-01 it must become a subcommand of the binary itself (`agent-brain guard --policy <file>`), so the hook command in `settings.json` is `<current_exe> guard --policy ...`. (c) `--no-session-persistence` exists in `claude` 2.1.301 `--help` ("only works with --print") and S8 showed it leaves no transcript, so D-15's primary path is the flag; deleting `<config_dir>/projects/*/<session_id>.jsonl` (+ sibling `<session_id>/` dir) is a defence-in-depth fallback. (d) `--max-turns` is **not listed** in `claude --help` 2.1.301 although S8 (2.1.300) used it successfully — with `--tools ""` a single turn is guaranteed anyway; keep the flag but treat it as needing one live confirmation.

Static musl needs user-approved installs the executor (agy) may not do: `rustup target add x86_64-unknown-linux-musl` (network) and, preferably, `sudo apt install musl-tools` (password; `sudo -n` fails here). Without musl-gcc, `cc-rs` fails (`failed to find tool "x86_64-linux-musl-gcc"`); a verified no-sudo fallback is host `gcc` with `-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -DSQLITE_DISABLE_LFS`, and a no-install, no-musl fallback is a static-glibc build (`-C target-feature=+crt-static`, also verified). All crates must be declared in the first plan and fetched once (`cargo fetch`, user-approved network step); afterwards `cargo build --offline --locked` works for the executor (verified, also with a 103 MB `cargo vendor` tree and an empty `CARGO_HOME`).

**Primary recommendation:** One crate `agent-brain/` at repo root (edition 2021, tokio `current_thread`, all 9 runtime deps + `tempfile` pinned in plan 03-01, fetched once), pure modules first under the shared `tests/contract/*.json` fixtures, then `db.rs` and `runner.rs` (kill = record descendants → SIGTERM group → grace → SIGKILL group + verified sweep), then roles/fast-path/stats, then a `scripts/build-release.sh` musl build and a user live-acceptance checklist for P2-01..P2-09.

## Architectural Responsibility Map

This is a local single-user CLI, so "tiers" are layers inside one process plus the OS and external CLI.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Arg parsing, exit codes, terminal event lines | CLI layer (`main.rs`, `terminal sink`) | — | Presentation only; must be separable from core so Phase 5 web server reuses core |
| Routing, scan/add, plan/QA parsing, profile env | Core library (pure functions) | — | Deterministic, contract-tested against JSON fixtures; no I/O except explicit fs/git helpers |
| Spawn/kill/supervise `claude` | OS process layer (`runner.rs`) | Core (emits `RunEvent`s) | Owns pgid, signals, /proc inspection, timeouts; knows nothing about SQLite or terminal |
| Stream-json parse, usage/turn summary | Core (`stream.rs`) | Storage | Pure bytes→event classification; storage and display are consumers |
| Persistence (projects mirror, tasks, runs, events) | Storage (`db.rs`, SQLite WAL) | — | Single DB, two processes (`run` and `kill`) need busy_timeout |
| Project catalogue (`projects_map.json`) | Storage (file, source of truth D-05) | Storage (SQLite mirror) | Hand-edited by user; atomic write, preserve unknown keys and key order |
| Git facts (status/log/diff, branch ops) | OS process layer (`git` subprocess via `gitops.rs`) | Core | Same as Phase 2; never shells, args as array |
| Dev read/write sandbox | Hook process (`agent-brain guard`) invoked by claude | Core (`guard.rs` pure decision fn) | Enforced outside the model; fail-closed |
| Token/cost statistics | Storage (SQL aggregates) | CLI layer | `runs.profile` snapshot makes STAT-01 a WHERE clause |
| Distribution | Build (musl static) | — | Compile-time concern; verified by script |

## Project Constraints (from CLAUDE.md)

- Claude plans and reviews; **Antigravity (`agy`) executes** the implementation. Plans must be executable by another agent without the conversation: exact files, steps, acceptance criteria, verify commands. [CITED: /mnt/data/IDE/QuangVy-Orchestration/CLAUDE.md]
- agy runs: stdin `/dev/null`, stdout to a file, explicit `--print-timeout`, outer `timeout`; no turn-limit flag; tools needing approval are **soft-denied while agy still exits 0** — always check the diff. `--dangerously-skip-permissions` only with user approval (approved for Phase 2 per memory; re-confirm for Phase 3). [CITED: CLAUDE.md, MEMORY.md]
- Kill method for agy runs: record descendants, SIGTERM the recorded PID, sweep; never `pkill`/`killall`/kill by name. The same rule applies to the Rust tests and live checks written here (signal only pids the test spawned).
- agy prompt must include: plan path, repo root + branch, commit per task (no `git add -A`, no `--no-verify`), what not to touch (`.planning/STATE.md`, `ROADMAP.md`, untracked `.planning/` files, `~/.claude-orca`), verify commands, summary contents.
- Review checklist: diff inside declared files; re-run verify commands; acceptance criteria met; no secrets (`spikes/lib/scan-secrets.sh` where applicable).
- **Implication for this phase:** executor sessions must not install anything or use the network. Every network/install step (below, "User-approval checkpoints") must be a `checkpoint:human-action` task the user performs, and the plans must not contain `cargo add`/`cargo update`/first-time `cargo build` without a prior `cargo fetch`.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap (derive) | 4.6.7 | CLI parsing | Named in spec/PROJECT.md [VERIFIED: PROJECT.md Constraints + Cargo.lock from prototype build]; MSRV 1.85 [VERIFIED: sparse index `rust_version` 1.85] |
| serde + serde_json (`preserve_order`) | 1.0.229 / 1.0.154 | JSON for map, config, stream events, hook payload | Named in spec. `preserve_order` is required: `projects_map.json` must keep key order and unknown keys (`R-unknown-keys` fixture), and `serde_json::Value` is the soft-parse type |
| rusqlite (`bundled`) | 0.40.2 (libsqlite3-sys 0.38.2, SQLite 3.53.2) | SQLite | Named in spec; compiled and ran WAL + migrations on musl [VERIFIED: probe output `sqlite 3.53.2`, `journal=wal`] |
| tokio (`rt`,`process`,`io-util`,`signal`,`time`,`macros`,`sync`) | 1.53.3 | Async child + signals + timers | Named in PROJECT.md Constraints (also Phase 5 Axum needs it). Use the **current-thread** runtime (`#[tokio::main(flavor = "current_thread")]`) |
| libc | 0.2.190 | `kill`, `killpg`, `getpgrp`, `prctl` | Already a transitive dep; 4 tiny unsafe calls wrapped in safe fns (ran in prototype) |
| unicode-normalization | 0.1.25 | NFD for accent stripping, NFC for path compare | std has no normalization; `is_combining_mark` used to drop marks; 8/8 `normalize.json` cases pass [VERIFIED: probe4 run] |
| regex | 1.13.1 | Plan/QA markdown parsing (meta comment, verdict line cleanup, separators) | Python used `re` with DOTALL/IGNORECASE; hand-porting is error-prone. NB: no look-behind (see Don't Hand-Roll) |
| chrono (`default-features=false`, `clock`,`std`) | 0.4.45 | Local `HH:MM:SS` for terminal lines, ISO-8601 UTC for meta | Avoids hand-written calendar code and unsafe `localtime_r`; verified output `11:36:29` local, `2026-10-16T04:36:29Z` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile (dev-dependency) | 3.27.0 | Temp dirs/repos for integration tests | All Rust tests; resolved and fetched in prototype |

No other crates. Specifically **not** used: anyhow/thiserror (a 15-line `BrainError { message, code }` mirrors Python), nix (libc suffices), signal-hook/ctrlc (tokio::signal suffices), assert_cmd (`env!("CARGO_BIN_EXE_agent-brain")` + `std::process::Command` suffices), toml (config is JSON like the rest), sha2/humantime/shlex/which/dirs/url.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tokio current_thread | std threads + `signal-hook` | Fewer deps and no async, but contradicts PROJECT.md "Tokio" constraint and Phase 5 will need tokio anyway; async pitfalls are neutralised by the reader-task + mpsc pattern below. **Flagged in Open Questions (Q1)** since the user cannot read Rust |
| libc | nix 0.31.3 | Safe wrappers, one more crate; no functional gain here |
| chrono | `time` 0.3 | `UtcOffset::current_local_offset()` refuses to work when other threads exist (soundness guard) — avoid |
| rusqlite `bundled` + musl | system libsqlite3 | Not static; violates DIST-01 |
| musl target | static glibc (`+crt-static` on gnu target) | Needs no rustup target/musl-gcc (verified working) but deviates from locked D-04; offered as fallback only |

**Installation (executor does NOT run these — see checkpoints):**

```bash
# plan 03-01 creates agent-brain/Cargo.toml with ALL dependencies below, then the USER runs once:
cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain && cargo fetch --locked   # network
# thereafter the executor uses: cargo build --offline --locked / cargo test --offline --locked
```

```toml
# Source: prototype scratchpad probe4/Cargo.toml (built and run 2026-10-16)
[package]
name = "agent-brain"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"

[dependencies]
clap = { version = "4.6", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
rusqlite = { version = "0.40", features = ["bundled"] }
tokio = { version = "1.53", features = ["rt", "process", "io-util", "signal", "time", "macros", "sync"] }
libc = "0.2"
unicode-normalization = "0.1"
regex = "1"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }

[dev-dependencies]
tempfile = "3"

[profile.release]
strip = true
lto = "thin"          # keep panic = "unwind" (default): Drop guards must run to kill the child group
```

**Version verification:** versions above are the resolved `Cargo.lock` entries of a prototype build with rustc 1.98.1 on 2026-10-16 (clap 4.6.7, serde 1.0.229, serde_json 1.0.154, rusqlite 0.40.2, libsqlite3-sys 0.38.2, tokio 1.53.3, libc 0.2.190, chrono 0.4.45, regex 1.13.1, unicode-normalization 0.1.25, tempfile 3.27.0, cc 1.8.0). `~/.cargo/registry` already caches older tokio 1.53.1/libc 0.2.189/serde_json 1.0.151 but **not** clap, rusqlite, chrono, regex, unicode-normalization or tempfile, so a network fetch is unavoidable. A 85-package lockfile resulted for the runtime set; release static binary of the prototype is ~5 MB stripped.

**rusqlite API gotchas found by compiling:** `usize` does not implement `FromSql` (needs feature `fallible_uint`) — read `PRAGMA user_version` as `i64`. `Connection::busy_timeout`, `pragma_update`, `pragma_query_value`, `transaction`, `execute_batch`, `query_row`, `params!` compile as shown in Code Examples.

## Package Legitimacy Audit

Run via `gsd-tools query package-legitimacy check --ecosystem crates ...` on 2026-10-16.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| clap | crates.io | 11 yrs (2015-03) | 19.9M/wk | github.com/clap-rs/clap | OK | Approved (named in spec) |
| serde | crates.io | 11 yrs | 28.3M/wk | github.com/serde-rs/serde | OK | Approved |
| serde_json | crates.io | 11 yrs | 28.2M/wk | github.com/serde-rs/json | OK | Approved (named in spec) |
| rusqlite | crates.io | 11 yrs | 3.4M/wk | github.com/rusqlite/rusqlite | OK | Approved (named in spec) |
| tokio | crates.io | 10 yrs | 20.2M/wk | github.com/tokio-rs/tokio | OK | Approved (named in spec) |
| libc | crates.io | 11 yrs | 32.2M/wk | github.com/rust-lang/libc | OK | Approved (transitive of tokio) |
| chrono | crates.io | 12 yrs | 15.7M/wk | github.com/chronotope/chrono | OK | Approved; name chosen from training knowledge -> `[ASSUMED]` provenance, seam OK |
| regex | crates.io | 12 yrs | 21.9M/wk | github.com/rust-lang/regex | OK | Approved; same note |
| unicode-normalization | crates.io | 11 yrs | 9.2M/wk | github.com/unicode-rs/unicode-normalization | OK | Approved; same note |
| tempfile | crates.io | 11 yrs | 16.3M/wk | github.com/Stebalien/tempfile | OK | Approved (dev-only); same note |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
Crate build scripts: `libsqlite3-sys` compiles bundled `sqlite3.c` via the `cc` crate (expected; see musl section). No `postinstall` concept for crates; no network access in the build scripts observed during prototype builds beyond registry downloads.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo / rustc | everything | yes | 1.98.1 (2026-09-01) | — |
| rustup | adding musl target | yes | installed; only `x86_64-unknown-linux-gnu` target | — |
| `x86_64-unknown-linux-musl` std | DIST-01 (D-04) | **no** | — | needs `rustup target add` (network, writes `~/.rustup`). Verified equivalent: rust-std tarball extracted into a toolchain copy |
| `musl-gcc` / `x86_64-linux-musl-gcc` | bundled SQLite C compile for musl | **no** | apt candidate `musl-tools 1.2.4-2` | (a) user runs `sudo apt install musl-tools` (password needed, `sudo -n true` fails); (b) host `gcc` 13.3.0 + CFLAGS hack (verified); (c) static-glibc build (verified) |
| gcc / cc | C compile of SQLite | yes | 13.3.0 (Ubuntu), glibc 2.39 | — |
| python3 | running `tests/fakes/fake_claude.py` in Rust tests, running the 114 Python tests | yes | 3.12.3 | — |
| git | gitops, test repos | yes | 2.43.0 | — |
| claude CLI | live checks only (never in research/auto tests) | yes | 2.1.301 | tests use fakes |
| network (for the user steps) | `cargo fetch`, `rustup target add` | yes (verified: crates.io/index/static.rust-lang.org reachable) | — | — |
| `readelf`, `file`, `ps`, `setsid` | verify scripts | yes | — | — |
| docker | optional "scratch image" run of the static binary | yes | 29.8.1 | `env -i` check |

**Missing dependencies with no fallback:** none (all have a route; the preferred musl route needs two user-approved steps).
**Missing dependencies with fallback:** musl std + musl-gcc (see above).

### User-approval checkpoints (agy must not do these)

1. `cd agent-brain && cargo fetch --locked` — network; populates `~/.cargo/registry` (once, after plan 03-01 pins every dependency; re-run only if dependencies change).
2. `rustup target add x86_64-unknown-linux-musl` — network, modifies `~/.rustup`.
3. `sudo apt install musl-tools` — sudo + network. Optional if the CFLAGS fallback or static-glibc is accepted.
4. Live acceptance runs that spend subscription tokens (P2-01, P2-02 real `sleep 300`, P2-07 real `inspect`, one real task, personal profile run).
5. Installing the binary on PATH (`install -m755 ... ~/.local/bin/agent-brain`) and uninstalling the Python symlink (`bash brain/install.sh --uninstall`) per D-02.
6. `--dangerously-skip-permissions` for agy (memory says approved for Phase 2 only) — re-confirm.

## Architecture Patterns

### System Architecture Diagram

```
                       user terminal (SIGINT/SIGHUP)                 other terminal
                              │                                           │
   argv ──► main.rs (clap) ───┼─► routing/projects ─► projects_map.json   │  agent-brain kill <task_id>
              │               │        (Fallback Ask: print list, exit 2) │          │
              │               │                                           │          ▼
              ▼               ▼                                    ┌──────────── db.rs ◄───────────┐
        role flow (lead|dev|qa|inspect)                            │ projects|tasks|runs|events    │
   preflight: require_runnable, profile, git dirty/branch          │ (WAL, busy_timeout, migrate)  │
        │                                                          └───────▲───────────┬───────────┘
        ▼                                                                  │ INSERT/UPDATE       │ SELECT running
  build argv + env (profile.rs) ─► runner.rs                               │                     ▼
   settings.json/policy.json ──────►  spawn claude  ──(process_group(0))──┤       reconcile at startup:
   (hook = agent-brain guard)         │  pid==pgid; record /proc starttime │       alive? (pid+starttime) else 'lost'
                                      │                                    │
        stdout bytes ──► reader task ─┴─► mpsc<Vec<u8>> ─► stream.rs ──► RunEvent{kind, summary, raw}
        stderr ──► runs_dir/<..>/stderr.txt                       │            │
        timeout / SIGINT x2 / SIGTERM / SIGHUP / `kill` ──────────┘            ├─► terminal sink (one line per event, summary line)
              │                                                                ├─► db.events (raw_json verbatim, bad lines too)
              ▼                                                                └─► run summary → db.runs (tokens from `result`, else partial)
   kill_tree(): record descendants (/proc) → SIGTERM group → wait grace → SIGKILL group + verified SIGKILL of recorded survivors
              │
              ▼
   post-checks (git changed paths vs plan, QA verdict, commit) → exit code (0/1/2/3/4/130)

   claude ──hooks──► `agent-brain guard --policy policy.json` (stdin payload, exit 2 = block) ──► guard.jsonl
```

### Recommended Project Structure

```
agent-brain/                      # new crate at repo root (edition 2021)
├── Cargo.toml, Cargo.lock        # lock committed
├── assets/                       # include_str! sources (copies of brain/prompts + brain/templates + default config)
│   ├── prompts/{lead,dev,qa}.md
│   ├── templates/{plan,qa-report}.md
│   └── config.default.json
├── src/
│   ├── main.rs        # clap, dispatch, exit-code mapping (try_parse -> map usage errors)
│   ├── lib.rs         # pub mod ... so integration tests/Phase 5 can reuse the core
│   ├── error.rs       # BrainError{message, code}; EXIT_* consts
│   ├── config.rs      # load JSON config (+embedded default), paths (XDG), env overrides
│   ├── profile.rs     # build_env(profile, cfg, base_env), check_profile        (PROF-02)
│   ├── routing.rs     # normalize, slugify, resolve                              (CLI-02)
│   ├── projects.rs    # map load/save(atomic, order-preserving), scan, add, duplicates
│   ├── planfile.rs    # parse_plan, validate_plan, read/set_meta, qa verdict     (CLI-03)
│   ├── gitops.rs      # git() wrapper + changed_paths/branch/commit helpers
│   ├── guard.rs       # decide(policy, payload) + write_hook_settings + `guard` entry
│   ├── stream.rs      # line reader, classify, Summary (turns, usage, denials, edits)  (LOG-01/02)
│   ├── db.rs          # open+PRAGMAs, MIGRATIONS, mirror projects, runs/events/stats queries
│   ├── runner.rs      # spawn, supervise (select loop), kill_tree, reconcile, proc helpers
│   ├── roles.rs       # lead/dev/qa flows (ports of role_*.py), tool allowlists, abs_rules
│   ├── fastpath.rs    # status (git only), inspect (1 turn + session cleanup)
│   ├── stats.rs       # aggregation + table/--json output
│   └── term.rs        # terminal sink: one-line formatting, EPIPE-safe writes
└── tests/
    ├── common/mod.rs  # Sandbox (tempdir HOME/XDG/DB/config), fake claude launcher, git helpers
    ├── contract_*.rs  # read ../tests/contract/*.json via CARGO_MANIFEST_DIR
    ├── runner_kill.rs, stream_fixtures.rs, cli_e2e.rs, db_stats.rs, fastpath.rs, dist.rs
scripts/build-release.sh          # musl static build + static assertions (new, repo-level)
tests/contract/stream.json        # NEW golden file (see Validation) ; tests/contract/guard.json NEW
tests/fakes/fake_claude_tree.py   # NEW fake: grandchild in new session, SIGTERM modes
```

Rationale: spec §4.3 lists `main, projects, runner, stream, db, fastpath, profile`; the extra modules are the Phase 2 modules that must be ported anyway. `lib.rs` keeps the core independent of printing (CONTEXT: "runner/stream/db nên tách khỏi phần in terminal").

### Pattern 1: Reader task + mpsc + select supervisor (cancel-safe)

**What:** One tokio task reads child stdout as **bytes** (`read_until(b'\n')`) and sends each complete line over an unbounded mpsc channel; the supervisor `select!`s on `rx.recv()`, `child.wait()`, three signal streams and a timeout. **Why:** `read_until` leaves partial data in the caller's buffer if cancelled (cancel-safe if the buffer persists), `read_line` is *not* cancel-safe and `lines()` errors on invalid UTF-8 and drops the line [VERIFIED: tokio-1.53.1 `async_buf_read_ext.rs` "Cancel safety" docs read this session]. The channel recv is cancel-safe. After `child.wait()` returns, drain the channel with a short deadline (≈2 s) and abort the reader task: an orphaned grandchild that inherited stdout would otherwise keep the pipe open and EOF never comes (Python had the same hazard and used `reader_thread.join(timeout=5)` [VERIFIED: brain/agent.py:229 `reader_thread.join(timeout=5)`]).

### Pattern 2: Kill = record → SIGTERM group → grace → SIGKILL group + verified sweep

Verified in the prototype (`probe2`, musl static, 5 scenarios, all `survivors=[]`):

1. `descendants(root_pid)` from `/proc/*/stat` ppid map **before** any signal (S4: "ghi lại PID claude và toàn bộ hậu duệ … TRƯỚC khi gửi tín hiệu").
2. `killpg(root, SIGTERM)` only if `root.pid > 1`, `root.pid != getpgrp()` and `/proc/<pid>/stat` pgrp field == pid (child is the leader of its own group). Never `killpg` a group that is not provably the child's.
3. Poll every 50 ms until root and all recorded procs are gone, or `kill_grace_secs` (default 3) elapses, or a **second SIGINT** arrives.
4. If still alive: `killpg(root, SIGKILL)` **and** SIGKILL each recorded descendant that still matches pid+starttime (these live in other sessions, S4). No second TERM/2 s step: 3 s grace + 2 s sweep would exceed the P2-02 5-second budget; SIGKILL is immediate.
5. `child.wait()` to reap; recheck survivors; report.

Observed timings: cooperative SIGTERM 85 ms; SIGTERM ignored + 2 s grace → 2.04 s total; double SIGINT → 0.40 s. `kill_on_drop(true)` stays as the second layer, but it only SIGKILLs the *direct* child; add a `Drop` guard that calls the same group+descendant SIGKILL if the supervisor exits abnormally (panic keeps unwinding because `panic = "abort"` is not set).

### Pattern 3: Run row lifecycle and PID-reuse-safe identity

Store `(pid, pgid, pid_start)` where `pid_start` = field 22 of `/proc/<pid>/stat` (clock ticks since boot). A process is "the same" iff `pid_start` matches **and** state is not `Z`/`X`. `kill <task_id>` and startup reconciliation (D-12) use the same `alive(ProcId)`; a mismatching start time means PID reuse → mark `lost`, signal nothing. Insert the `runs` row *after* a successful spawn (pid known); if the INSERT fails, kill the child group immediately (never leave an unrecorded child). `kill` (a second process) runs the same `kill_tree` itself and sets `kill_requested_at`; the owning `run` process sees the child exit and records `killed` instead of `failed`. If the owner is dead but the group is alive, `kill` still works because it only needs pid+starttime from the DB.

### Pattern 4: Hook command is the binary itself

`write_hook_settings` writes `settings.json` with the **nested** shape and `policy.json`/`guard.jsonl` in the run dir; `command` = `'<current_exe>' guard --policy '<policy.json>'` (single-quote shell quoting). Block = exit code 2 + stderr text; malformed policy/stdin = exit 2 (fail closed) [VERIFIED: brain/guard.py:103-127 returns 2 on missing `--policy`, unreadable policy, empty/invalid stdin].

### Pattern 5: Terminal sink never panics

`println!` panics on EPIPE/EIO — exactly what happens when the terminal is closed (SIGHUP). All output goes through `term::emit(line)` that ignores write errors. Truncate summaries by `chars()`, never byte slicing (Vietnamese multi-byte).

### Anti-Patterns to Avoid

- **`killpg` of an unverified pgid** (could be the supervisor's own group or init's): always check `pgrp == pid`, `pid > 1`, `pid != getpgrp()`.
- **Relying on group kill alone** — S4: claude's Bash child is in its own session; only claude's own SIGTERM handler cleans it, and SIGKILL orphans it.
- **`pkill`/name-based kill in tests or code** (CLAUDE.md): signal only recorded pids, verified by start time.
- **Reading stdout with `lines()`/`read_line`**: errors on invalid UTF-8, loses the line, not cancel-safe.
- **Using the real home in tests** — Phase 2 had a test that wrote run dirs into the real `~/.local/share/agent-brain/runs` [CITED: 02-07-SUMMARY.md "tasklog test … writes `proj-1/task-fail` run dirs into the real"]; the Rust sandbox must `env_clear()` and set `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`.
- **Python hook in settings.json** (breaks DIST-01 on a machine without python3).
- **Two claude processes on one personal config dir** (E-06 unverified OAuth rotation): refuse to start a second `running` run with the same profile when profile is `personal`; warn for `work`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Arg parsing/help | manual argv loop | clap derive | subcommand tree, value enums (verified skeleton compiles; usage error exit = 2) |
| JSON with order + unknown keys | struct-only serde for the map | `serde_json::Value` + `preserve_order` | `R-unknown-keys` fixture requires unknown keys survive; Python dict order must be kept |
| Unicode NFD/NFC | table of Vietnamese letters | `unicode-normalization` (+ explicit `đ/Đ→d`) | covers all Mn marks; matches fixture |
| Markdown comment/verdict regexes | ad-hoc `find` loops | `regex` | DOTALL lazy and IGNORECASE semantics from the Python code |
| Signal delivery to async loop | `libc::signal` handlers | `tokio::signal::unix` | async-signal-safety handled; 3 streams verified |
| SQLite | custom file store | rusqlite bundled | WAL, concurrent `run`+`kill` |
| Calendar/timezone | manual civil-date math | chrono (`clock`) | |
| Temp dirs in tests | manual cleanup | tempfile | |

**Acceptable small hand-rolls (each guarded by a test):** `/proc/<pid>/stat` parse (find the *last* `)` because `comm` may contain `)`/spaces; fields after it start at field 3), POSIX `normpath` (Python `posixpath.normpath` semantics for `plan.json`: `.` → `.`, `./x` → `x`, collapses `..`), "non-strict realpath" (canonicalize the longest existing ancestor then append the remainder; `std::fs::canonicalize` fails on non-existent targets and the guard/validate code needs Python `os.path.realpath`/`Path.resolve()` semantics), URL credential stripping for `http(s)://user:pass@host` (fixture `R-remote-credentials`), `sh_quote`, PRAGMA-`user_version` migration loop, markdown-table cell split on `|` not preceded by `\` (Rust `regex` has **no look-behind**; Python used `(?<!\\)\|`), `--since 7d` parser.

**Key insight:** the risky logic (routing, plan validation, guard decisions, stream summary) is small and fully pinned by fixtures; the genuinely tricky OS behaviour (kill tree, PID reuse, static link) is not available in any crate that fits the constraints, so it is hand-written once, in `runner.rs`, behind tests that spawn real process trees.

## Common Pitfalls

### Pitfall 1: Group kill leaves claude's Bash children alive
**What goes wrong:** `killpg(claude)` returns OK but `sleep` survives. **Why:** S4 — "Tiến trình Bash con của claude và `sleep` nằm trong session và nhóm tiến trình RIÊNG (sid = pgid = PID của bash con)"; with SIGKILL on claude "`sleep 318` … SỐNG SÓT" [CITED: spikes/spike-results.md S4]. **Avoid:** record descendants first, sweep survivors by pid+starttime (Pattern 2). **Detect:** test with `fake_claude_tree.py` that starts its grandchild with `start_new_session=True` and (mode `ignore`) ignores SIGTERM.

### Pitfall 2: Spec's 5-second budget vs. grace + sweep
**What goes wrong:** 3 s grace followed by a 2 s TERM→KILL sweep (the S4 shell routine) = 5 s, too tight. **Avoid:** after grace expiry send SIGKILL to group and survivors at once (done in prototype); keep `kill_grace_secs` configurable.

### Pitfall 3: Clap usage errors collide with Fallback Ask (exit 2)
**Verified:** `probe4 run bogus a` exits **2** with a clap usage message, and `EXIT_ROUTING = 2` is also the Fallback Ask code. Python's argparse had the same overlap. **Avoid:** use `Cli::try_parse()` and map clap *usage* errors to exit 1 (keep `--help`/`--version` at 0); document it. Not covered by any fixture, so it is a safe improvement.

### Pitfall 4: Closed terminal => EIO/EPIPE panic in `println!`
**Why:** SIGHUP closes the tty; the next `println!` panics, skipping the kill path. **Avoid:** `term::emit` swallows errors; register the SIGHUP stream before spawning; the Drop guard still kills the group on panic (unwind).

### Pitfall 5: Invalid UTF-8 or a truncated last line
SIGKILLed claude (S4 r2) leaves a stream with no `result` and possibly a half line. **Avoid:** `read_until` bytes, `String::from_utf8_lossy`, treat EOF-without-newline as a line; non-object JSON (`[1, 2]`) is a bad line like in Python [VERIFIED: tests/fixtures/stream/malformed_lines.jsonl lines `not json`, `[1, 2]`, a truncated `{"type": "system", "sub`].

### Pitfall 6: One argument over 131072 bytes => E2BIG
**Verified:** `/bin/true` with a 131072-byte argument fails `Errno 7 Argument list too long`; 131000 works (`ARG_MAX` is 2097152, the per-argument limit is what bites). `inspect` embeds the diff in `-p "<prompt>"` (S8 variant B), so cap the diff (default ~100,000 bytes incl. fixed prompt text, configurable `inspect_max_diff_bytes`), cut on a line boundary, append `[diff cắt: đã gửi N/M byte]`, and print the same notice on the terminal (D-14).

### Pitfall 7: `session_id` from the stream used as a file name
The fallback cleanup deletes files by `session_id`. **Avoid:** accept only `^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$`, glob `<config_dir>/projects/*/<id>.jsonl` and remove the sibling directory `<id>/` (observed: session dirs exist next to the `.jsonl`, e.g. `~/.claude/projects/-mnt-data-IDE-QuangVy-Orchestration/d2edd3e2-….jsonl` and `d2edd3e2-…/`). Directory name encoding seen: `/home/pc0741/.cache/qv-spikes/s1` -> `-home-pc0741--cache-qv-spikes-s1` (`/` and `.` -> `-`, hyphens kept); behaviour for `_` and non-ASCII characters is unobserved, so **do not reconstruct the encoding — glob by session id**.

### Pitfall 8: Reader never sees EOF
An orphan holding the inherited stdout pipe blocks `read_until` forever. **Avoid:** after `child.wait()`, drain with a deadline then abort the reader task (Pattern 1).

### Pitfall 9: rusqlite `usize`, long transactions, locked DB
`usize: FromSql` is not implemented (compile error seen). Use `i64`. `run` holds the DB open for minutes while `kill` writes: WAL + `busy_timeout(5000)`; no long write transactions; autocommit per event with `synchronous=NORMAL`.

### Pitfall 10: Hidden-flag drift — `--max-turns`
`claude --help` (2.1.301) has no `--max-turns` line (grep for "turns" returned nothing) while S8 on 2.1.300 proved the flag works (`error_max_turns`, "Reached maximum number of turns (1)"). Whether 2.1.301 still accepts it is **unobserved** (`[ASSUMED]` yes). `inspect` is safe either way because `--tools ""` removes all tools (one assistant message); verify `result.num_turns == 1` and `subtype == "success"` and treat anything else as failure. Add the flag to the first live check (a rejected flag exits non-zero immediately).

### Pitfall 11: A glibc-header C build linked against musl
`CC_x86_64_unknown_linux_musl=gcc` alone fails to link: `undefined reference to 'open64'`, `'__memmove_chk'`, `'stat64'` (output captured). It links only with `-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -DSQLITE_DISABLE_LFS`. Prefer real musl headers via `musl-gcc` (verified clean).

### Pitfall 12: Python/Rust shared `projects_map.json` drift
Both tools read/write the same file during the transition (D-02). Rust must write exactly what Python writes: `ensure_ascii=False`, 2-space indent, trailing `\n`, atomic temp+fsync+rename in the same dir [VERIFIED: brain/projects.py:98-117; test asserts `content.endswith(b"\n")`, no `\u` escapes], unknown keys and key order preserved. `serde_json::to_string_pretty` matches the indent; empty arrays print `[]` in both.

### Pitfall 13: `.plan.md` meta line format
Python writes `<!-- brain-meta: {…sorted keys, ", " and ": " separators, ">" for ">"…} -->` [VERIFIED: brain/planfile.py:31-34]. Only the *reader* matters for compatibility (it parses JSON), but keep keys sorted (use a typed struct in alphabetical order or `BTreeMap`) and replace `>` -> `>` so a `-->` can never appear inside the comment. Compare metas as parsed values, not strings (Lead may rewrite the line).

## Code Examples

All snippets below were compiled and executed (musl static or gnu) in the research scratchpad unless marked otherwise.

### Cargo/musl build (scripts/build-release.sh core)

```bash
# Route A (preferred, after user: rustup target add + sudo apt install musl-tools)
CC_x86_64_unknown_linux_musl=musl-gcc \
  cargo build --release --locked --offline --target x86_64-unknown-linux-musl
# Route B (no sudo; verified): host gcc with glibc headers
CC_x86_64_unknown_linux_musl=gcc \
CFLAGS_x86_64_unknown_linux_musl="-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -DSQLITE_DISABLE_LFS" \
  cargo build --release --locked --offline --target x86_64-unknown-linux-musl
# Route C (no musl at all; deviates from D-04): static glibc, verified
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --locked --offline --target x86_64-unknown-linux-gnu
# assertions the script must make:
file target/x86_64-unknown-linux-musl/release/agent-brain | grep -q 'static-pie linked\|statically linked'
! readelf -d target/x86_64-unknown-linux-musl/release/agent-brain | grep -q NEEDED
# Failure seen with no musl compiler: 'error occurred in cc-rs: failed to find tool "x86_64-linux-musl-gcc": No such file or directory'
```

Offline reproducibility, verified: `cargo vendor <dir>` (67 crates, 103 MB — do NOT commit) + `.cargo/config.toml` source replacement + `--frozen` built successfully with an **empty** `CARGO_HOME`; and with a populated `CARGO_HOME`, `cargo build --release --offline --locked` rebuilt in 1.45 s. Recommendation: commit `Cargo.lock` only; one user `cargo fetch`; no vendor dir in git.

### /proc helpers + kill routine (ran; condensed)

```rust
// Source: research scratchpad probe2/src/main.rs (musl static; scenarios coop/ignore/double-SIGINT/SIGHUP/SIGTERM all survivors=[])
struct Stat { state: char, ppid: i32, pgrp: i32, start: u64 }
fn read_stat(pid: i32) -> Option<Stat> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rp = s.rfind(')')?;                                   // comm may contain ')' and spaces
    let f: Vec<&str> = s.get(rp + 2..)?.split_ascii_whitespace().collect();
    // f[0]=state(field 3) f[1]=ppid f[2]=pgrp f[19]=starttime(field 22)
    Some(Stat { state: f.first()?.chars().next()?, ppid: f.get(1)?.parse().ok()?,
                pgrp: f.get(2)?.parse().ok()?, start: f.get(19)?.parse().ok()? })
}
fn alive(p: ProcId) -> bool {
    matches!(read_stat(p.pid), Some(s) if s.start == p.start && s.state != 'Z' && s.state != 'X')
}
fn signal_group(root: ProcId, sig: i32) {
    let ours = unsafe { libc::getpgrp() };
    if root.pid > 1 && root.pid != ours && alive(root)
        && read_stat(root.pid).map(|s| s.pgrp) == Some(root.pid) {
        unsafe { libc::killpg(root.pid, sig); }
    }
}
// spawn: Command::new(claude).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(<File>)
//        .process_group(0).kill_on_drop(true)   // tokio::process::Command::process_group exists in 1.53
```

### Supervisor `select!` skeleton (ran)

```rust
let mut sigint  = signal(SignalKind::interrupt())?;
let mut sigterm = signal(SignalKind::terminate())?;
let mut sighup  = signal(SignalKind::hangup())?;
let mut sigs = 0;
loop {
    tokio::select! {
        line = rx.recv() => { /* classify, print, INSERT events */ }
        st = child.wait() => break st?,
        _ = sigint.recv()  => { sigs += 1; if sigs == 1 { kill_tree(root, grace, &mut child, &mut sigint).await; } }
        _ = sigterm.recv() => { kill_tree(root, grace, &mut child, &mut sigint).await; }
        _ = sighup.recv()  => { kill_tree(root, grace, &mut child, &mut sigint).await; }
        _ = &mut timeout   => { /* status timed_out; same kill_tree */ }
    }
}
// inside kill_tree: second sigint.recv() during the grace wait => forced = true (SIGKILL now)
```

### Normalize / slugify (ran against `tests/contract/normalize.json`: 8 cases, 0 mismatches)

```rust
use regex::Regex; use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
pub fn normalize(text: &str) -> String {
    let s: String = text.replace(['đ', 'Đ'], "d").nfd().filter(|c| !is_combining_mark(*c)).collect();
    Regex::new(r"[\s_\-]+").unwrap().replace_all(&s.to_lowercase(), " ").trim().to_string()
}
```
(Python `unicodedata.category(c) != "Mn"` drops only Mn; `is_combining_mark` drops Mn/Mc/Me — no difference for Vietnamese; build the regexes once with `std::sync::OnceLock`.)

### SQLite open + migrations (ran twice on the same file: `user_version=2 sum=Some(5)` then `Some(10)`)

```rust
const MIGRATIONS: &[&str] = &[ /* 0001 schema … */, /* 0002 ALTER … */ ];
pub fn open(path: &str) -> rusqlite::Result<Connection> {
    let mut c = Connection::open(path)?;
    c.busy_timeout(Duration::from_millis(5000))?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    c.pragma_update(None, "synchronous", "NORMAL")?;
    c.pragma_update(None, "foreign_keys", "ON")?;
    let current: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;   // i64, not usize
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = c.transaction()?; tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?; tx.commit()?;
    }
    Ok(c)
}
```
Create the DB file with mode 0600 and its directory 0700 before opening (secrets can appear in tool results stored in `events.raw_json`).

### Claude command lines (the protocol constants to port)

From `brain/agent.py:96-124` [VERIFIED: read this session] — argv order: `claude_bin`, `-p`, `<user_input>`, `--output-format`, `stream-json`, `--verbose`, `--model`, `<models[role]>`, `--append-system-prompt`, `<prompt text>`, then `common_args` (`--strict-mcp-config`), optional `--max-turns N`, role `extra_args` (`--settings <file>` for Dev), `--allowedTools …`, `--disallowedTools …`. It rejects any arg starting `--dangerously-skip-permissions` / `--allow-dangerously-skip-permissions` / `--permission-mode` or containing `bypassPermissions`.

`inspect` (S8 variant B + live help): `claude -p "<prompt+diff>" --output-format stream-json --verbose --model sonnet --max-turns 1 --tools "" --no-session-persistence --strict-mcp-config` (the empty string is a real argv element). `--no-session-persistence` help text: "Disable session persistence - sessions will not be saved to disk and cannot be resumed (only works with --print)"; `--tools`: "Use \"\" to disable all tools" [VERIFIED: claude --help 2.1.301].

## Port Map (Phase 2 contract values, quoted verbatim)

Exit codes [VERIFIED: brain/__init__.py:3-8]:
```
EXIT_OK = 0
EXIT_ERROR = 1
EXIT_ROUTING = 2
EXIT_PROTOCOL = 3
EXIT_BLOCKED = 4
EXIT_INTERRUPTED = 130
```
Routing failures print to **stdout**, all other errors to **stderr** [VERIFIED: brain/cli.py:91-98 `if e.code == EXIT_ROUTING: sys.stdout.write(...) else: sys.stderr.write(...)`].

Nested-session env to strip [VERIFIED: brain/config.py:9-21]: `"CLAUDECODE"`, `"CLAUDE_CODE_CHILD_SESSION"`, `"CLAUDE_CODE_SESSION_ID"`, `"CLAUDE_PID"`, `"CLAUDE_EFFORT"`, `"CLAUDE_VC_ENV"`, `"CLAUDE_CODE_MESSAGING_SOCKET"`, `"CLAUDE_CODE_MESSAGING_TOKEN"`, `"CLAUDE_CODE_SESSION_ATTENDED"`, `"CLAUDE_CODE_ENTRYPOINT"`, `"CLAUDE_CODE_EXECPATH"`; and always `env["CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR"] = "1"` (config.py:115). Profile env: `unset_env` removed, `claude_config_dir` null -> unset `CLAUDE_CONFIG_DIR`, `~` expanded against `env["HOME"]` (config.py:101-113).

Default config [VERIFIED: brain/config.json]: `"claude_bin": "claude"`, models `lead: "opus"`, `dev: "sonnet"`, `qa: "sonnet"`; profiles `work: {claude_config_dir: null, unset_env: [ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL]}`, `personal: {claude_config_dir: "~/.claude-orca", …}`; `common_args: ["--strict-mcp-config"]`; `timeout_seconds` lead 1800, dev 1800, qa 1200. New keys for Rust: `kill_grace_secs: 3`, `inspect_max_diff_bytes`, `inspect_model: "sonnet"`.

Plan/QA constants [VERIFIED: brain/planfile.py:10-12]:
```
PLAN_SECTIONS = ("Mục tiêu", "Thay đổi", "Không được đụng tới", "Cách kiểm tra", "Vướng mắc")
QA_SECTIONS = ("Thay đổi có khớp plan không (từng mục)", "Sửa ngoài plan (nếu có)", "Rủi ro", "Kết luận")
QA_VERDICTS = ("Đạt", "Cần sửa")
```
`validate_plan` error codes, in the exact order produced (the fixture compares the **ordered list**): `missing_title`, `missing_section:<name>`, `bad_row:<n>`, `empty_file_cell:<n>`, `no_changes`, then per file in order `absolute_path:`, `home_path:`, `parent_traversal:`, `git_dir:`, `protocol_file:`, `not_a_file:`, `glob_path:`, `outside_root:` (planfile.py:181-267; `plan.json` has 24 cases). `plan-outside-root` needs a materialised `root_layout` with a symlink to a file outside the repo (the Python test builds `repo/`, `outside/target.py`, and a symlink; the Rust test harness must do the same).

Role tools [VERIFIED: role_lead.py:17-34, role_dev.py:25-46, role_qa.py:19-36]: Lead allow = `Read, Grep, Glob, Write(./.plan.md), Edit(./.plan.md), Bash(git log *), Bash(git show *), Bash(git diff *), Bash(git status *)` + abs rules; Lead deny = `WebFetch, WebSearch, NotebookEdit, Task`. Dev allow = per plan file `Read(./f) Edit(./f) Write(./f)` + abs rules + `Bash(git add *)`, `Bash(git commit *)`; Dev deny = `Grep, Glob, WebFetch, WebSearch, NotebookEdit, Task`. QA allow = `Read, Grep, Glob, Write(./.qa-report.md), Edit(./.qa-report.md), Bash(git diff *)`, `git log *`, `git show *`, `git status *` + abs rules; QA deny = `WebFetch, WebSearch, NotebookEdit, Task`. `abs_rules` format: `f"{tool}(/{abs_path})"` where `abs_path` already begins with `/`, i.e. `Write(//abs/path)` (agent.py:38-50; live-confirmed A-11 [CITED: 02-07-SUMMARY.md]).

Hook settings shape [VERIFIED: brain/guard.py:74-83]: `{"hooks": {"PreToolUse": [{"matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob", "hooks": [{"type": "command", "command": cmd}]}]}}`. Guard policy keys: `root`, `read_allow`, `write_allow`, `log` (guard.py:58-63). guard.jsonl entry keys: `ts`, `tool`, `path`, `decision`, `reason` (guard.py:22-28).

Run dir [VERIFIED: brain/agent.py:53-56 and brain/config.py:64-70]: `base / project["id"] / task / f"{ts}-{role}"` with `ts = %Y%m%dT%H%M%S%fZ`; `base` = `cfg["runs_dir"]` or `$XDG_DATA_HOME/agent-brain/runs` or `~/.local/share/agent-brain/runs`; directories chmod `0o700`. Map path [VERIFIED: brain/projects.py:15-19]: `$XDG_CONFIG_HOME/agent-brain/projects_map.json` else `~/.config/agent-brain/projects_map.json`.

Messages the contract harness asserts (keep byte-identical, with the program name adjusted only where no fixture reads it): `CẢNH BÁO: tên/alias trùng "<key>":` followed by the ids (scan.json harness, test_projects.py), `Chưa có scan_roots — chạy: brain scan --root <thư mục>` (exit 1), `Thư mục không tồn tại: <abs>` (exit 1), `CẢNH BÁO: thư mục gốc không tồn tại, bỏ qua: <path>`, `Đã quét N thư mục gốc`, `(trống) — chạy: brain scan --root <thư mục>`, `Không tìm thấy dự án "x"`. Rust's own e2e tests may use `agent-brain` in those strings; if the planner wants Python's 114 tests to be reusable against the binary, the strings must stay `brain ...` — recommended **not** to chase that (see Validation Architecture).

## State of the Art

| Old approach | Current approach | When changed | Impact |
|--------------|------------------|--------------|--------|
| Spec: "kill the group, `kill_on_drop` as backup" | Record descendants first + verified sweep (S4) | Phase 1 (2026-10-15) | `kill_on_drop` only reaches the direct child; Bash tools run in other sessions |
| `claude -p` with Python hook script | Hook = `agent-brain guard` subcommand | this phase | DIST-01 (no Python required) |
| `inspect` via "let claude run git diff" (S8 variant A: `error_max_turns`, exit 1, no text) | Embed diff, `--tools ""`, `--no-session-persistence`, `--max-turns 1` | S8 | 1 turn, no transcript |
| `rusqlite` features pre-0.3x (`usize` readable) | `fallible_uint` needed for `usize` | rusqlite 0.4x | read as `i64` |
| `std::env::home_dir` deprecated | No deprecation warning on rustc 1.98.1 (compiled) | recent std | OK to use with `$HOME` first |

**Deprecated/outdated:** reading `lines()` from tokio for untrusted streams; `Command::pre_exec` for `setpgid` (use `process_group(0)`, safe since std 1.64 / tokio exposes it — [VERIFIED: tokio-1.53.1 src/process/mod.rs:790 `pub fn process_group(&mut self, pgroup: i32)`]).

## Suggested Data Model (Claude's discretion, D-05..D-08, D-12)

```sql
-- migration 0001 (PRAGMA user_version = 1)
CREATE TABLE projects (                    -- mirror of projects_map.json (D-05); rows never deleted
  id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE,   -- = map "id"
  path TEXT NOT NULL, name_vi TEXT NOT NULL DEFAULT '', aliases_json TEXT NOT NULL DEFAULT '[]',
  type TEXT NOT NULL DEFAULT 'code', profile TEXT NOT NULL DEFAULT '', repo TEXT, missing INTEGER NOT NULL DEFAULT 0);
CREATE TABLE tasks (
  id INTEGER PRIMARY KEY,                  -- the short integer shown as "task 42" (D-08)
  project_id INTEGER NOT NULL REFERENCES projects(id), slug TEXT NOT NULL, title TEXT NOT NULL,
  branch TEXT NOT NULL, base_branch TEXT, base_commit TEXT,
  status TEXT NOT NULL DEFAULT 'open',     -- open|planned|dev_done|blocked|qa_pass|qa_fix
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(project_id, branch));
CREATE TABLE runs (
  id INTEGER PRIMARY KEY, task_id INTEGER REFERENCES tasks(id),          -- NULL for inspect without a task branch
  project_id INTEGER NOT NULL REFERENCES projects(id),                   -- denormalised for stats by project
  role TEXT NOT NULL CHECK (role IN ('lead','dev','qa','inspect')),
  engine TEXT NOT NULL DEFAULT 'claude', profile TEXT NOT NULL,          -- snapshot at run time (STAT-01)
  model TEXT, session_id TEXT, pid INTEGER, pgid INTEGER, pid_start INTEGER,
  status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','timed_out','killed','lost')),
  outcome TEXT,                            -- qa:pass|qa:fix|dev:blocked|protocol_violation (Phase 6 ENG-07 wants this)
  exit_code INTEGER, kill_requested_at INTEGER, started_at INTEGER NOT NULL, ended_at INTEGER,  -- epoch ms
  input_tokens INTEGER NOT NULL DEFAULT 0, output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0, cache_write_tokens INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0, num_turns INTEGER, assistant_turns INTEGER,
  turns_before_first_edit INTEGER, permission_denials INTEGER NOT NULL DEFAULT 0,
  usage_source TEXT NOT NULL DEFAULT 'none' CHECK (usage_source IN ('result','partial','none')),
  base_commit TEXT, run_dir TEXT, error TEXT);
CREATE INDEX runs_started ON runs(started_at);
CREATE INDEX runs_running ON runs(status) WHERE status = 'running';
CREATE TABLE events (
  id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  seq INTEGER NOT NULL, ts INTEGER NOT NULL,
  kind TEXT NOT NULL,                      -- system/init | system/<subtype> | assistant | user | result | rate_limit_event | unknown:<type> | bad_json
  raw_json TEXT NOT NULL, UNIQUE(run_id, seq));
```

Stream -> columns (field names verified in `spikes/s1/raw/s1.jsonl`): `session_id` from `system/init` (also on every line); `usage.input_tokens`, `usage.output_tokens`, `usage.cache_read_input_tokens`, `usage.cache_creation_input_tokens` from the final `type: result` line -> `input/output/cache_read/cache_write`; `total_cost_usd` -> `cost_usd`; `num_turns`; `permission_denials` (array length); `subtype`, `is_error`, `terminal_reason`. Turn count = distinct `message.id` over `type: assistant` lines (Phase 2 semantics: S1 has 3 assistant lines but 2 ids and `num_turns: 2`). **Partial usage (killed / no `result`, as in `s4-r2.jsonl`):** sum, over distinct `message.id`, the last-seen `message.usage` fields and set `usage_source='partial'`; S1 notes per-line `output_tokens` are provisional (16, 6, 6 vs 293 total), so output is an under-estimate; input/cache fields add up exactly in S1 (11168 + 19219 = 30387 cache read). On SIGTERM claude still prints a `result` (S4 r1, r3), so most `killed` runs have exact usage.

Event kinds seen in real fixtures (unknown kinds must be stored, not rejected): `system/{hook_started,hook_response,init,thinking_tokens,background_tasks_changed,task_started,task_notification,task_updated}`, `assistant`, `user`, `rate_limit_event`, `result/success` (S1 + S4 files).

## Validation Architecture

> `workflow.nyquist_validation` is absent in `.planning/config.json` (only `workflow._auto_chain_active` is set) -> treated as enabled.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in, Rust 1.98.1) + `tempfile`; Python `unittest` stays as regression for shared fixtures |
| Config file | none — Wave 0 creates `agent-brain/Cargo.toml` |
| Quick run command | `cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain && cargo test --offline --locked --lib` |
| Full suite command | `cd /mnt/data/IDE/QuangVy-Orchestration/agent-brain && cargo test --offline --locked && cd .. && python3 -m unittest discover -s tests -t . -q` |
| Static/dist check | `bash scripts/build-release.sh` (asserts static + runs `env -i` copy test) |

Test locations/fixture discovery: Rust tests find shared files with `Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/contract/<file>.json")`, the fake with `../tests/fakes/fake_claude.py` (mode 100755 in git [VERIFIED: `git ls-files -s` -> `100755 … tests/fakes/fake_claude.py`]) and stream samples with `../spikes/s1/raw/s1.jsonl`, `../spikes/s4/raw/s4-r{1,2,3}.jsonl`, `../tests/fixtures/stream/*.jsonl`. The binary under test is `env!("CARGO_BIN_EXE_agent-brain")`. Sandbox rule: `Command::env_clear()` then set only `PATH`, `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `AGENT_BRAIN_DB`, `AGENT_BRAIN_CONFIG`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM=1`, `FAKE_CLAUDE_RECORD`, `FAKE_CLAUDE_SCENARIO` (names from tests/helpers.py:52-56 and fake_claude.py:49,64).

### Fake-claude strategy

Reuse `tests/fakes/fake_claude.py` unchanged through config `claude_bin` (Phase 2 did the same via `BRAIN_CONFIG`). Its scenario keys [VERIFIED: fake_claude.py:72-84]: `stderr`, `sleep`, `raw_lines`, `events`, `ops` (`write`/`read`/`grep`/`git`), `ignore_hooks`, `exit_code`; default stream = `system/init` (session `fake-session`, model `fake`), tool events, `result` with usage 10/5/0/0 and cost 0. It runs PreToolUse hooks from `--settings` via `shell=True`, so it exercises the **Rust** `agent-brain guard` subcommand end to end. Two additions are needed (new files; do not edit the Phase 2 fake):
- `tests/fakes/fake_claude_tree.py` — prints a `system/init` line, starts `bash -c 'sleep 317'` with `start_new_session=True` (like claude's Bash tool), writes `"<fake pid> <grandchild pid>"` to `$PIDFILE`, mode env `MODE=coop` (on SIGTERM terminate the child, exit 143) or `MODE=ignore` (ignore SIGTERM), then idles. This reproduces S4 r1 and r2 deterministically. (Prototype `scratchpad/probe2/fake_tree.py` is the working draft.)
- A `session_file` behaviour (in a new `fake_claude_inspect.py` or the same tree fake): create `<CLAUDE_CONFIG_DIR>/projects/x/<session_id>.jsonl` and `<session_id>/` to prove the D-15 fallback deletes them.
Wrap the fake for config use if exec bit/shebang is a concern: `claude_bin` may be a generated `#!/bin/sh` wrapper `exec python3 <abs>/fake_claude.py "$@"`.

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-02 | normalize/slug (8), routing (9), scan/add (10 multi-step) identical to Python | contract | `cargo test --offline --locked contract_` | Wave 0 (`tests/contract_*.rs`) |
| CLI-03 | plan (24) + qa_report (8) cases; prompts/templates byte-equal to `brain/prompts`, `brain/templates`; role argv/allowlists equal (dry-run output) | contract + unit | `cargo test --offline --locked planfile roles` | Wave 0 |
| CLI-03 | Lead/Dev/QA e2e with fake: Lead writes only `.plan.md` + commit `<task>: plan`; Dev out-of-plan write -> guard blocks / exit 3; blocked plan -> exit 4; QA verdict + commit `<task>: qa`; dirty tree stops | integration | `cargo test --offline --locked --test cli_e2e` | Wave 0 |
| PROF-02 | 7 `profile_env.json` cases + fake records `claude_config_dir` per profile; empty/unknown profile exit 1 and fake NOT invoked | contract + integration | `cargo test --offline --locked profile` | Wave 0 |
| CLI-01 / P2-01 | event lines printed per stream event + final summary line (turns, tokens, time, status) | integration | `cargo test --offline --locked --test cli_e2e run_prints_steps` | Wave 0 |
| PROC-01 / P2-02 | `kill <task_id>` from a second process; within 5 s none of the recorded pids (fake + grandchild in new session) alive; both modes `coop` and `ignore`; run row `killed` | integration (real process trees) | `cargo test --offline --locked --test runner_kill -- --test-threads=1` | Wave 0 |
| PROC-02 / P2-03 | SIGINT once (graceful), SIGINT twice (immediate KILL < 1 s), SIGTERM, SIGHUP to the `run` process => no survivors, status `killed`, tokens kept | integration | same test binary | Wave 0 |
| PROC-01 safety | reconcile: stale `running` row with dead pid -> `lost`; PID reused (start time mismatch) -> `lost` and **no signal sent** (test spawns a decoy process with another start time) | integration | `cargo test --offline --locked reconcile` | Wave 0 |
| LOG-01 / P2-04 | after fake run: runs row has tokens 10/5/0/0, session_id `fake-session`, pid>0, pgid, started<ended, status; events count == stdout lines; each `raw_json` byte-equal to the line | integration (reads SQLite) | `cargo test --offline --locked --test cli_e2e db_row` | Wave 0 |
| LOG-01 | real-format summary on `s1.jsonl` / `s4-r1,r3` / truncated `s4-r2` (golden numbers below) | unit/contract | `cargo test --offline --locked stream_fixtures` | Wave 0 (+ new `tests/contract/stream.json`) |
| LOG-02 / P2-05 | `raw_lines` scenario with `not json`, `[1, 2]`, truncated JSON, unknown fields, invalid UTF-8 bytes (unit-level) -> exit 0, bad lines stored raw as `bad_json`, `result` still recorded | integration + unit | `cargo test --offline --locked bad_lines` | Wave 0 |
| FAST-01 / P2-06 | `status` with `claude_bin=/nonexistent` succeeds; prints branch/status/log/diffstat; `runs` row count unchanged; no FAKE_CLAUDE_RECORD file | integration | `cargo test --offline --locked --test fastpath status_zero_token` | Wave 0 |
| FAST-02 / P2-07 | recorded argv contains `--max-turns 1`, `--tools ""` (empty element), `--no-session-persistence`, `--model sonnet`; one `runs` row `role=inspect`, `num_turns=1`; diff cap notice when > cap; fallback deletes fake-created session file+dir; invalid `session_id` never touches fs | integration | `cargo test --offline --locked --test fastpath inspect_` | Wave 0 |
| STAT-01 / P2-08 | seeded DB with work+personal runs: `stats --profile work --json` totals equal work-only sums; by-project/by-role tables; `--since 7d` window | integration | `cargo test --offline --locked --test db_stats` | Wave 0 |
| DIST-01 / P2-09 | musl build is static (`file`, no `NEEDED`); copy to fresh dir, `env -i HOME=<tmp> PATH=/nonexistent ./agent-brain stats` exits 0 | build/system | `bash scripts/build-release.sh` | Wave 0 (script) |

**Golden numbers for `tests/contract/stream.json`** (computed with Python `brain.streamlog.summarize` this session; Rust must equal them): `s1.jsonl`: session `1794dfc4-1acc-4f78-afc7-5dd721fe31d6`, model `claude-opus-5-5`, assistant_turns 2, tool_counts `{Bash:1}`, usage in/out/cache_read/cache_creation = 4/293/30387/10482, num_turns 2, cost 0.0958094, denials 0, 9 lines, 0 bad. `s4-r1.jsonl`: 16 lines, turns 2, usage 4/289/38450/2434, cost 0.032958. `s4-r3.jsonl`: 17 lines, usage 4/252/32013/8840, cost 0.0821786. `s4-r2.jsonl` (SIGKILLed): 12 lines, assistant_turns 1, `format_ok=false` (no result), partial usage from the single message id = in 2 / out 8 / cache_read 12788 / cache_creation 6437. `tests/fixtures/stream/malformed_lines.jsonl`: `bad_lines == 3` and result present. Plan 03-01 should create this JSON (and `guard.json`: ported cases from `tests/test_guard.py` — read allowed, denied, relative path, `..` traversal, symlink outside, Edit allowed/outside, MultiEdit/NotebookEdit path keys, Grep/Glob blocked when `read_allow` set, read-free mode, non-guarded tool, fail-closed on malformed input/policy, NFC/NFD) and make a small Python test load them too, so D-03's "both languages read the same files" holds for stream and guard as well.

### Sampling Rate
- **Per task commit:** `cargo test --offline --locked --lib` (pure modules, < 30 s) plus the single touched integration test.
- **Per wave merge:** full `cargo test --offline --locked` + `python3 -m unittest discover -s tests -t . -q` (114 Python tests must stay green; they also gate the fixture edits).
- **Phase gate:** full suite green + `scripts/build-release.sh` green + user live checklist signed, before `/gsd-verify-work`.

### Live (user) checks — cannot be automated (consume tokens, need a real terminal or the real CLI)

| ID | Check | Why manual |
|----|-------|------------|
| P2-01 | `agent-brain run lead "<throwaway project>" "<request>"` on a real project; read the per-step lines | real `claude`, tokens |
| P2-02 (real) | real `claude` running `sleep 300` through the role pipeline, `kill <task_id>`, wait 5 s, `ps` marker check | needs permission to run `sleep`; suggested route: alternate `AGENT_BRAIN_CONFIG` whose `claude_bin` is a wrapper adding `--allowedTools "Bash(sleep *)"` and a request like S4's "Run this exact bash command and wait for it: sleep 31N" (S4 prompt shape verified). How multiple `--allowedTools` occurrences merge is **unobserved** — verify with the dry-run argv first |
| P2-03 (real terminal) | Ctrl+C in a real terminal (once, twice) and closing the terminal window; `ps` afterwards | real tty/session semantics; the automated test delivers the same signals with `kill()` |
| P2-07 (real) | count `*.jsonl` under `~/.claude/projects/<encoded cwd>/` before/after one `inspect`; confirm `num_turns == 1`; confirm `--max-turns` is still accepted by 2.1.301 | real CLI |
| D-02 parity | one real task Lead->Dev->QA with the binary on a non-tool repo | real CLI, then user removes the Python symlink |
| E-06 | personal profile (`~/.claude-orca`) one run when no other session uses it (never run live in Phase 2) | shares login with the orchestrating session |
| P2-09 (machine) | copy binary to another directory/machine | optional; automated `env -i` test covers the directory case |

## Security Domain

`security_enforcement` is not set in config -> enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (local single-user CLI; no auth added) | — |
| V3 Session Management | no | — (claude sessions are the CLI's; only deletion by validated id) |
| V4 Access Control | yes | Dev read/write sandbox = `agent-brain guard` hook (fail-closed, exit 2) + `--allowedTools/--disallowedTools` + post-run git check (3 layers carried from Phase 2 README §7) |
| V5 Input Validation | yes | `session_id` UUID regex before any path use; slug via `slugify`; plan paths via `validate_plan`; project path from map must be a git repo root; argv built as arrays (no shell) for git and claude; hook command single-quoted |
| V6 Cryptography | no | — (do not hand-roll; none needed) |
| V7 Error handling/logging | yes | never log env; `raw_json` may contain tool output -> run dir `0700`, DB `0600`, parent dir `0700` |
| V12 Files/Resources | yes | atomic map write (temp+fsync+rename), no symlink-following writes outside root (guard `realpath`), `include_str!` assets (no runtime file reads for prompts) |
| V14 Config | yes | reject `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`, `--permission-mode`, `bypassPermissions` in configured/extra args (port `build_command` check) |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Signalling a reused PID (kills an unrelated process) | Tampering/DoS | pid + `/proc` start-time identity, zombie check, `pgrp == pid` check (verified design) |
| `killpg` of own group/init | DoS | guards `pid > 1`, `!= getpgrp()` |
| Path traversal via stream-provided `session_id` | Tampering | UUID regex + glob under the profile `projects/` dir only |
| Dev escapes plan sandbox via Grep/Glob/Bash redirection | Elevation | guard hook blocks Grep/Glob when `read_allow` set; Bash limited to `git add`/`git commit`; post-run diff check |
| Command injection through project/request strings | Injection | `Command::arg`, never `sh -c`; hook command quoting function tested with quotes/spaces in paths |
| SQL injection | Injection | `params!` everywhere; no string-built SQL except constant DDL |
| Secrets in logs/DB | Info disclosure | restrictive modes; do not store env; document that `events.raw_json` mirrors the stream (Phase 6 ENG-05 adds scrubbing) |
| Two claude processes on one OAuth store (E-06) | Availability | refuse concurrent runs per personal profile; warn for work |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `claude` 2.1.301 still accepts `--max-turns` although `--help` does not list it (S8 proved it on 2.1.300) | Pitfall 10, inspect | `inspect` exits non-zero immediately; fix = drop the flag (`--tools ""` already forces 1 turn). One live check settles it |
| A2 | `PR_SET_PDEATHSIG(SIGTERM)` in `pre_exec` (optional third layer, only safe on the current-thread runtime where the spawning thread is the main thread) makes claude die when the supervisor is SIGKILLed | Pattern 2 note | None if omitted; if included, needs its own test (kill -9 the supervisor, assert fake gets SIGTERM). Not prototyped |
| A3 | Tokio `current_thread` runtime is acceptable to the user although the user cannot read Rust; PROJECT.md names Tokio | Standard Stack, Q1 | Alternative std-threads design is smaller but contradicts PROJECT.md |
| A4 | Names `chrono`, `regex`, `unicode-normalization`, `tempfile` were chosen from training knowledge (legitimacy seam = OK, compile verified) | Package Audit | None technically; flagged per provenance rule |
| A5 | Claude Code's per-project directory encoding for characters other than `/` and `.` (underscore, non-ASCII) is unknown | Pitfall 7 | Avoided by globbing on `session_id` |
| A6 | `thinking_tokens`, `background_tasks_changed`, `task_*` system subtypes are the only unknown subtypes in 2.1.301 | Data model | Parser stores any unknown kind; no loss |
| A7 | Real `claude` closes its inherited stdout in children (grandchildren do not hold our pipe) | Pitfall 8 | Mitigated by drain deadline either way |
| A8 | Rust's musl target in 1.98.1 bundles musl >= 1.2.4 (missing `open64`/`stat64` symbols seen at link time are consistent with it) | Pitfall 11 | Affects only the hack route, which is verified on this machine |
| A9 | Multiple `--allowedTools` occurrences merge in claude's parser | Live check P2-02 | Use dry-run argv; fall back to a single combined value |
| A10 | `sudo apt install musl-tools` is acceptable to the user (alternatives B/C exist) | Environment | Choose route B or C |

## Open Questions

1. **Tokio (async) vs. plain threads** — The user cannot read Rust and spec §7 lists "lỗi async khó hiểu" as a risk; PROJECT.md nevertheless fixes Tokio and Phase 5 (Axum) needs it.
   - What we know: the reader-task + mpsc + `select!` pattern prototyped cleanly on a current-thread runtime.
   - What's unclear: whether the user wants to override the constraint.
   - Recommendation: keep Tokio (current-thread), isolate all async in `runner.rs`; everything else synchronous and unit-testable.

2. **`run dev|qa` positional request** — Success criterion #1 writes `run lead|dev|qa "<dự án>" "<yêu cầu>"`, but Dev/QA take no request in Phase 2 (Dev "only reads the plan").
   - Recommendation: request is required for `lead`, optional and ignored (stderr note) for `dev`/`qa`, so the protocol boundary stays intact.

3. **Adoption of in-flight Python tasks** — branches `task/<slug>` created by `brain lead` have no `tasks` row.
   - Recommendation: `run dev/qa/status/inspect` lazily create the row from `.plan.md` meta (`task`, `branch`, `base_commit`, `request`); otherwise D-02 transition tasks break.

4. **Protocol task log (`protocol-log/*.md`, `python3 -m brain.tasklog check`)** — not in any Phase 3 requirement; DB `runs` supersedes it.
   - Recommendation: do not port; keep the Python gate for the 5-task evidence; store `turns_before_first_edit` and `reads_outside_plan` count in `runs` and print them in the summary line (CONTEXT: KPI "số turn trước khi sửa code").

5. **musl route** — A (sudo + rustup target), B (rustup target + host gcc CFLAGS), or C (static glibc, no installs, deviates from D-04)?
   - Recommendation: A, with `scripts/build-release.sh` auto-falling back to B when `musl-gcc` is absent; C only if the user waives D-04.

6. **How to drive real `claude` into `sleep 300` through Lead/Dev/QA for the live P2-02 check** — roles do not allow `sleep`.
   - Recommendation: user-run script with an alternate config whose `claude_bin` is a wrapper adding `--allowedTools "Bash(sleep *)"`; settle merge behaviour with the dry-run argv first (A9).

7. **Where should canonical prompts live?** Recommendation: copy into `agent-brain/assets/` and keep a byte-equality test against `brain/prompts/*.md` and `brain/templates/*.md` while `brain/` exists; if the user later edits a Rust prompt, delete the parity test deliberately.

## Plan Slicing Suggestion (for the planner; waves are not mandatory)

- **03-01 (Wave 0, checkpoint):** crate skeleton with every dependency pinned, `lib.rs`/module stubs, `error.rs`, `tests/common`, `.gitignore` (`agent-brain/target/`, any vendor dir), new fixtures `stream.json`, `guard.json`, new `fake_claude_tree.py`, `scripts/build-release.sh` skeleton; **user checkpoint:** `cargo fetch --locked` (+ `rustup target add`, optionally `sudo apt install musl-tools`).
- **Wave 1 (parallel, pure):** (a) routing+projects+config/profile; (b) planfile; (c) stream summary + reader; (d) guard + hook settings; (e) gitops.
- **Wave 2 (parallel):** `db.rs` (schema, migrations, mirror, reconcile, stats SQL); `runner.rs` (spawn, kill_tree, signals) with real-process tests.
- **Wave 3:** `roles.rs` + `main.rs` + terminal sink (Lead/Dev/QA flows end to end with fake).
- **Wave 4:** `fastpath.rs` (status, inspect), `kill`, `stats`, reconcile hook-up.
- **Wave 5:** release build + static assertions; user live-acceptance checklist P2-01..P2-09 + D-02 parity task; uninstall Python symlink and install the binary (user steps).

Each module is small, owned by one plan, and verified by an automated command so the non-Rust-reading user reviews by behaviour table (CONTEXT: acceptance by P2-01..P2-09).

## Sources

### Primary (HIGH confidence — read or executed this session)
- `/mnt/data/IDE/QuangVy-Orchestration/brain/*.py`, `brain/prompts/*.md`, `brain/templates/*.md`, `brain/config.json`, `brain/README.md` — Phase 2 reference implementation (quoted with line ranges above).
- `/mnt/data/IDE/QuangVy-Orchestration/tests/contract/*.json` (case ids enumerated), `tests/helpers.py`, `tests/fakes/fake_claude.py`, `tests/test_{projects,planfile,profile,streamlog,guard}.py`, `tests/fixtures/stream/*.jsonl`.
- `/mnt/data/IDE/QuangVy-Orchestration/spikes/spike-results.md` (S1, S4, S5, S8, SPK-10), `spikes/s1/raw/s1.jsonl`, `spikes/s4/raw/*`.
- `/mnt/data/IDE/QuangVy-Orchestration/.planning/phases/02-protocol-mvp-scripts/{02-CONTEXT,02-07-SUMMARY,02-VERIFICATION}.md`; `.planning/{PROJECT constraints, REQUIREMENTS, ROADMAP, STATE}.md`; `files/03-phase2-loi-rust.md`.
- `claude --help` (2.1.301) — flags `--no-session-persistence`, `--tools`, `--settings`, `--strict-mcp-config`, `--allowedTools`; `--max-turns` absent from the text.
- tokio 1.53.1 source in `~/.cargo/registry/src` (`process_group`, `read_until`/`read_line` cancel-safety docs).
- Executed prototypes (scratchpad `probe`, `probe2`, `probe3`, `probe4`): musl static build with bundled SQLite (WAL file test), cc-rs failure output, glibc-header link failure output and CFLAGS fix, musl headers via extracted `musl-tools` debs (clean build), static-glibc build, `cargo vendor` + empty-`CARGO_HOME` `--frozen` build, supervisor kill scenarios, normalize fixture parity, clap exit code, rusqlite migrations, E2BIG measurement.
- crates.io sparse index / API for versions; `gsd-tools query package-legitimacy check` (all OK).

### Secondary (MEDIUM confidence)
- `~/.claude/projects/*` directory listing (encoding and sibling session dirs observed, not specified by documentation).

### Tertiary (LOW confidence)
- Items tagged `[ASSUMED]` in the Assumptions Log (A1, A2, A5-A10).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every crate resolved, compiled and exercised on rustc 1.98.1 incl. musl.
- Architecture / process control: HIGH — kill algorithm run against real process trees incl. new-session grandchildren; live `claude` specifics rely on Phase 1/2 observations.
- Distribution: HIGH for the three build routes; the "preferred" route's `musl-gcc` was exercised via extracted headers rather than an installed package.
- Pitfalls: HIGH where marked verified; MEDIUM for live-CLI behaviours (`--max-turns` in 2.1.301, A9).

**Research date:** 2026-10-16
**Valid until:** 2026-11-15 (crate versions move weekly; `claude` flags move with each release — re-run `claude --help` and `cargo fetch` results before executing)
