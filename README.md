# quang-vy-ai-orchestration

**Rust Agent Brain**: bộ điều phối (orchestrator) nhẹ, chạy bằng 1 file binary (`agent-brain`), dùng để giao việc cho nhiều AI agent (Claude, DeepSeek, Antigravity…) trên mọi dự án: code, video, khách hàng. Mục tiêu là thay thế Paperclip.

Quy trình phối hợp cốt lõi: **Lead → `.plan.md` → Dev → QA**.

Dự án được quản lý bằng [GSD Core](https://github.com/open-gsd/gsd-core) (spec-driven development cho Claude Code).

## Cấu trúc repo

```
.
├── files/          # Tài liệu gốc: roadmap tổng + spec từng phase (tiếng Việt)
├── .planning/      # Trạng thái GSD: PROJECT, REQUIREMENTS, ROADMAP, STATE, intel
├── .claude/        # GSD Core cài local cho Claude Code (agents, commands, hooks)
└── README.md
```

| File | Nội dung |
|---|---|
| `.planning/PROJECT.md` | Tầm nhìn, phạm vi, nguyên tắc, quyết định chính |
| `.planning/REQUIREMENTS.md` | 65 requirement v1 (truy vết về test case gốc `Pn-xx`) |
| `.planning/ROADMAP.md` | 6 phase, tiêu chí thành công, phụ thuộc |
| `.planning/STATE.md` | Phase hiện tại, blocker, câu hỏi còn mở |
| `.planning/INGEST-CONFLICTS.md` | Báo cáo xung đột khi tổng hợp tài liệu gốc |

## Cài đặt

Yêu cầu: [Claude Code](https://claude.com/claude-code), Node.js, Git.

```bash
git clone git@github.com:vinhhx/quang-vy-ai-orchestration.git
cd quang-vy-ai-orchestration

# Cài lại GSD để sinh .claude/settings.local.json (hook của GSD, không commit)
npx @opengsd/gsd-core@1.16.0 --claude --local
```

Sau đó mở Claude Code trong thư mục repo (`claude`). Các lệnh `/gsd-*` sẽ có sẵn.

## Lộ trình

| GSD Phase | Phase gốc | Nội dung | Spec gốc |
|---|---|---|---|
| 1 | 0 | Kiểm chứng giả định: thử 8 spike S1–S8, ghi `spike-results.md` | `files/01-phase0-kiem-chung.md` |
| 2 | 1 | Giao thức MVP bằng script: Lead → Dev → QA trên dự án thật | `files/02-phase1-giao-thuc-mvp.md` |
| 3 | 2 | Lõi Rust CLI: kill sạch cây tiến trình, log stream-json, token trong SQLite, fast-path | `files/03-phase2-loi-rust.md` |
| 4 | 3 | Tối ưu session & token dựa trên số liệu thật | `files/04-phase3-toi-uu-session.md` |
| 5 | 4 | Dashboard nhúng: Web UI + SSE trong cùng binary | `files/05-phase4-dashboard.md` |
| 6 | 5 | Đa engine & nhánh Media | `files/06-phase5-da-doi-hinh-media.md` |

Phụ thuộc: 1 → 2 → 3, sau đó Phase 4, 5, 6 độc lập với nhau (chỉ cần Phase 3).

> **Lưu ý đánh số:** GSD đánh số phase từ 1, nên GSD Phase = phase gốc + 1. Mã test case giữ số gốc, ví dụ `P2-02` (kill sạch) thuộc GSD Phase 3.

## Quy trình làm việc với GSD

Mỗi phase đi theo vòng: **discuss → plan → execute → verify**.

```text
/gsd-progress            # Xem đang ở đâu, nên làm gì tiếp
/gsd-discuss-phase N     # Làm rõ các điểm mập mờ trước khi lập plan
/gsd-plan-phase N        # Tạo PLAN.md chi tiết cho phase N
/gsd-execute-phase N     # Thực thi plan (commit nguyên tử từng task)
/gsd-verify-work         # Nghiệm thu theo tiêu chí (UAT)
```

Lệnh hữu ích khác:

```text
/gsd-quick               # Task nhỏ, vẫn có commit + theo dõi trạng thái
/gsd-pause-work          # Lưu ngữ cảnh khi dừng giữa chừng
/gsd-resume-work         # Khôi phục ngữ cảnh ở phiên sau
/gsd-help                # Danh sách đầy đủ các lệnh
```

Mẹo: chạy `/clear` trước mỗi lệnh lớn để có context sạch. Trạng thái nằm trong `.planning/` nên không mất gì.

**Bắt đầu:** `/gsd-plan-phase 1`

## Vấn đề còn mở

- Chính sách tài khoản công ty (Work/Personal) cần chốt trước Phase 3.
- Kết quả spike S7 (Antigravity có chạy headless không) quyết định phạm vi Phase 6.
- Warm reuse (`--resume`) chưa được gán task, cần chốt ở `/gsd-discuss-phase 4`.
- Phase 4 cần khoảng 2 tuần dữ liệu `runs` thật từ Phase 3.

Chi tiết xem `.planning/STATE.md`.
