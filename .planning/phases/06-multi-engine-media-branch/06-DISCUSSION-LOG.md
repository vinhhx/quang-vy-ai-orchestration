# Phase 6: Multi-engine & Media Branch - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-24
**Phase:** 6-multi-engine-media-branch
**Areas discussed:** Cách chạy DeepSeek / agy, Chọn đội hình ở đâu, Hết quota thì sao, Phạm vi nhánh Media, Điều phối việc cho nhân sự (user-added)

---

## Cách chạy DeepSeek / agy

| Question | Options | Selected |
|----------|---------|----------|
| DeepSeek path | claude + ANTHROPIC_BASE_URL / dsh container / both | claude + BASE_URL |
| Key source | env var / 0600 secrets file / both | env var |
| Antigravity | real headless engine / manual + git diff / both | real headless engine |
| Roles allowed on cheap engines | all but QA / Dev only / any with warning | all but QA |
| agy `--dangerously-skip-permissions` | Dev only + post-run diff check / every agy role / never | Dev only + diff check |
| If E-09 fails | stop and ask / auto-fallback to dsh | stop and ask |

## Chọn đội hình ở đâu

| Question | Options | Selected |
|----------|---------|----------|
| Where teams are declared | separate teams.json / inside workflows | **Other:** "Cấu hình trong view setting chủ động tạo được các team với mục đích khác nhau" → Settings screen |
| How a task gets a team | project default + override at assign / only at assign / only per project | project default + override |
| Claude engine vs profile | engine = project's profile / engines carry their own profile | engine = project's profile |
| Change team mid-task | allowed from next step / only on quota | allowed from next step |
| Settings screen edits | teams; engines fixed / teams + engines | teams; engines fixed |
| Team storage | teams.json written by UI / brain.db table | teams.json |
| Workflows on Settings too | no, read-only / yes | **yes** |

## Hết quota thì sao

| Question | Options | Selected |
|----------|---------|----------|
| Task state on quota | stop and wait for user / auto-retry same engine | stop and wait |
| Configurable auto-switch rules | no, always ask / yes per team | no, always ask |
| Notify via | dashboard + Telegram with buttons / Telegram notify only / dashboard only | dashboard + Telegram with buttons |
| What the new engine gets | plan + current diff / reset to plan commit | plan + current diff |

## Phạm vi nhánh Media

| Question | Options | Selected |
|----------|---------|----------|
| Whiteboard pipeline | not yet, generic job / user gives path | generic job |
| Split Media out | split into 06.1 / keep in 6 | split into 06.1 |
| Output location | inside media project folder / fixed shared folder | inside media project |

## Điều phối việc cho nhân sự (user-added area)

User's words: "Điều phối việc cho nhân sự ko cần phải thực hiện code nhưng vẫn sẽ review qua git diff hoặc live test mới thông qua."

| Question | Options | Selected |
|----------|---------|----------|
| Who and how | `người` engine in teams / only the user manually / non-code work | `người` engine |
| How results pass | existing workflow (AI review, QA, gate) / always "Bạn" gate | existing workflow |
| Staff access | none, user presses Xong / via Telegram | none |
| Where staff work | push branch to remote / this machine's worktree / both | push branch |

## Claude's Discretion

JSON shapes, how a task stores its team (migration as a decision gate), quota detection patterns per engine, Settings screen layout.

## Deferred Ideas

Auto-fallback rules; dsh black-box engine (only if E-09 fails); staff via Telegram/dashboard; editing engines on UI; Phase 06.1 Media Branch.
