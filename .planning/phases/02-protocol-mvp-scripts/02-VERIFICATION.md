---
phase: 02-protocol-mvp-scripts
verified: 2026-10-16T04:00:00Z
status: passed
score: 5/5 success criteria verified
behavior_unverified: 1
re_verification: false
gaps: []
human_verification:
  - test: "Personal profile (~/.claude-orca) end to end through brain"
    expected: "A personal-profile project's session transcript lands under ~/.claude-orca/projects/ and not under ~/.claude/projects/"
    why_human: "Not required by Phase 2 (PROF-01 covers work only) and not run live: it shares the login with the orchestrating session (risk E-06). Run it when no other session uses ~/.claude-orca."
---

# Phase 2 Verification: Protocol MVP (Scripts)

Goal: the user runs Lead → `.plan.md` → Dev → QA on real projects using only scripts and a config file.

Verified by the reviewer against the code (114 automated tests pass), the live run dirs under `~/.local/share/agent-brain/runs/`, the project repos' git history and the task logs in `protocol-log/`.

## Success criteria

| # | Criterion | Evidence | Status |
|---|-----------|----------|--------|
| 1 | Vietnamese name with diacritics or alias picks the right project; ambiguous or unknown names stop without running an agent | P1-01..P1-04 run live by the user (`nghiem-thu-p1.md`); live routing of "dự án apec", "dự án nháp", "app-sale-api", "messer", "karofi"; contract tests `tests/contract/routing.json` | passed |
| 2 | Scan adds new projects, keeps hand edits, warns on duplicate aliases | P1-05; live `brain scan` over 177 projects kept the hand-edited `QuangVy-Orchestration` entry and warned `core`/`core-2`, `onepage`/`onepage-2`; a missing root now exits 1 (fix `056ffde`) | passed |
| 3 | Lead writes only `.plan.md`; Dev edits only plan files and stops with Vướng mắc on a thin plan; QA writes only `.qa-report.md` | P1-06/07/09 commits in `du-an-nhap`; P1-08 `580e70e` exit 4 with only `.plan.md` changed; five real tasks: every Dev commit inside the plan, every QA commit only `.qa-report.md`; guard hook live (`guard.jsonl` written in every Dev run) | passed |
| 4 | `profile: work` runs with work's `CLAUDE_CONFIG_DIR` | P1-10: every live session id found under `~/.claude/projects/`, none under `~/.claude-orca/projects/` | passed |
| 5 | Five real tasks through the protocol, each with a complete log | `python3 -m brain.tasklog check protocol-log --min 5` → 5 complete, exit 0 (apec, app-sale-api, du-an-nhap, messer, karofi); KPIs in `02-07-SUMMARY.md` | passed |

## Defects found live and fixed in this phase

- Agent cwd drift made `Write(./.plan.md)` deny Lead's own plan (2 of 8 Lead runs). Fixed by `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1` plus absolute `//` rules (`35eb5e0`); confirmed live in the app-sale-api Dev run.
- `brain scan --root <missing>` accepted silently (fixed `056ffde`); a test wrote run dirs into the real home (fixed `fd4d08e`); manual gaps (fixed `6c10577`).

## Notes, not gaps

- QA cannot run tests by design (diff-only); in all five tasks it said so. The reviewer ran tests where the project allowed (du-an-nhap, messer).
- Dev turns before first edit were 1–4; the manual's ≤ 2 target was set on a toy repo.
- Three of the five judgment lines were written by Claude on the user's explicit delegation; the logs say so.
