# Fix plan: Phase 2 plans use the wrong personal profile directory

- Repo root: `/mnt/data/IDE/QuangVy-Orchestration`
- Branch: `main` (base commit `ed6ccae`)
- Type: documentation-only edit of planning files. No code, no tests, no `brain/` directory.
- Executor: Antigravity (`agy`). Reviewer: Claude.

## Why

The Phase 2 plans were written before Phase 1 ran. They set the **personal** Claude profile to `~/.claude-personal`, taken from Phase 1 decision D-06. Phase 1 then replaced that decision. The user's SPK-10 decision on 2026-10-15 is recorded in `spikes/spike-results.md` (section `## S5 — Tách profile`, line "Ảnh hưởng tới thiết kế", and the SPK-10 "Quyết định" line near the end of the file):

| Profile | Directory | `CLAUDE_CONFIG_DIR` | Account |
|---------|-----------|---------------------|---------|
| work | `~/.claude` | **unset** (never set explicitly, E-07) | company Team seat |
| personal | `~/.claude-orca` | `~/.claude-orca` | personal Pro, already logged in |

`~/.claude-personal` still exists on disk, but it is an unused, credential-free clone left over from spike S5. Nothing in Phase 2 may point at it.

`~/.claude-orca` is also the profile of the Claude session that orchestrates this project. Whether a second `claude` process on the same credentials makes the OAuth token rotate is **not verified** (E-06, S5 "Phương án thay thế" item 3). The plans must therefore (a) never create, copy or edit `~/.claude-orca`, and (b) tell the user to run personal-profile tasks only when no other Claude session is using `~/.claude-orca`.

## Scope

Edit exactly these six files and nothing else:

1. `.planning/phases/02-protocol-mvp-scripts/02-01-PLAN.md`
2. `.planning/phases/02-protocol-mvp-scripts/02-04-PLAN.md`
3. `.planning/phases/02-protocol-mvp-scripts/02-06-PLAN.md`
4. `.planning/phases/02-protocol-mvp-scripts/02-07-PLAN.md`
5. `.planning/phases/02-protocol-mvp-scripts/02-CONTEXT.md`
6. `.planning/phases/03-rust-cli-core/03-CONTEXT.md`

Do **not** touch:
- anything under `.planning/phases/01-verify-assumptions/` or `spikes/` (that is Phase 1's historical record, it correctly mentions `~/.claude-personal` as the S5 clone)
- `.planning/STATE.md`, `.planning/ROADMAP.md`, `.planning/state.json`, `.planning/config.json`, `.planning/research/`, this plan file
- any other text in the six files, including the "Phase 1 has not run" / "chưa tồn tại" wording and the A-01..A-10 assumption list (a separate follow-up handles those)
- the real directories `~/.claude`, `~/.claude-orca`, `~/.claude-personal` (do not read, list or modify them)

## Edits

Each edit gives the text to find and its replacement. The "find" text is copied from the file; match it exactly. If a "find" string is not present exactly once, stop and report it instead of guessing.

### 1. `02-01-PLAN.md`

**1a.** In `must_haves.flagged_assumptions` (A-06, around line 42):

Find:
```
; ~/.claude-personal exists (created in Phase 1 S5)"
```
Replace with:
```
; the personal profile is ~/.claude-orca (Phase 1 SPK-10 decision, spikes/spike-results.md S5), already logged in with the personal Pro account and also used by the orchestrating Claude session, so brain never creates, copies or edits that directory"
```

**1b.** In Task 1 `<read_first>` (around line 199), after the bullet that starts `- .planning/phases/01-verify-assumptions/01-RESEARCH.md "Pattern 1"`, insert this new bullet on its own line with the same indentation:
```
    - spikes/spike-results.md section "S5 — Tách profile" ("Ảnh hưởng tới thiết kế") and the SPK-10 "Quyết định" line: work = ~/.claude with CLAUDE_CONFIG_DIR unset, personal = ~/.claude-orca
```

**1c.** In Task 1 `<action>` step 5 (`brain/config.json`, around line 207):

Find:
```
`profiles.personal` = `{"claude_config_dir": "~/.claude-personal", "unset_env": [the same three]}` (Phase 1 D-06);
```
Replace with:
```
`profiles.personal` = `{"claude_config_dir": "~/.claude-orca", "unset_env": [the same three]}` (Phase 1 SPK-10 decision in spikes/spike-results.md; supersedes Phase 1 D-06);
```

### 2. `02-04-PLAN.md`

**2a.** In `<read_first>` (around line 142):

Find:
```
    - .planning/phases/01-verify-assumptions/01-CONTEXT.md D-06 (work = ~/.claude, personal = ~/.claude-personal)
```
Replace with:
```
    - spikes/spike-results.md S5 and the SPK-10 "Quyết định" line (work = ~/.claude with CLAUDE_CONFIG_DIR unset, personal = ~/.claude-orca; this supersedes Phase 1 D-06)
```

**2b.** In `<behavior>` (around line 152):

Find:
```
personal → equals "<HOME>/.claude-personal" expanded against the base env HOME;
```
Replace with:
```
personal → equals "<HOME>/.claude-orca" expanded against the base env HOME;
```

**2c.** In `<action>` step 3 (around line 163):

Find:
```
`P1-10-personal` (HOME `/home/u` → `/home/u/.claude-personal`)
```
Replace with:
```
`P1-10-personal` (HOME `/home/u` → `/home/u/.claude-orca`)
```

**2d.** In `<action>` step 6 (around line 168):

Find:
```
only when it contradicts D-08/D-09 or Phase 1 D-06.
```
Replace with:
```
only when it contradicts D-08/D-09 or the Phase 1 SPK-10 profile mapping (spikes/spike-results.md). Tests only ever use sandbox HOME directories; no test may read or write the real ~/.claude or ~/.claude-orca.
```

### 3. `02-06-PLAN.md`

**3a.** In Task `<action>` step 3, the `"Chuẩn bị"` bullet (around line 253):

Find:
```
pick a throwaway project for P1-06..P1-09 first.
```
Replace with:
```
pick a throwaway project for P1-06..P1-09 first. It also states the profile mapping (work = `~/.claude`, `CLAUDE_CONFIG_DIR` không đặt; personal = `~/.claude-orca`) and warns that `~/.claude-orca` is also the profile of the Orca/Claude session: run personal-profile tasks only from a terminal where no other Claude session is using `~/.claude-orca` (token rotation E-06 is unverified, spike S5).
```

**3b.** In the P1-10 sentence of the same step (around line 254):

Find:
```
no file with that id exists under `~/.claude-personal/projects/`.
```
Replace with:
```
no file with that id exists under `~/.claude-orca/projects/`.
```

### 4. `02-07-PLAN.md`

**4a.** In `must_haves.truths` (around line 26):

Find:
```
exists under ~/.claude/projects/ and not under ~/.claude-personal/projects/ (PROF-01
```
Replace with:
```
exists under ~/.claude/projects/ and not under ~/.claude-orca/projects/ (PROF-01
```

**4b.** In Task 2 `<precondition>` (around line 112):

Find:
```
(Phase 1 executed and approved; ~/.claude-personal exists)</precondition>
```
Replace with:
```
(Phase 1 executed and approved; the personal profile is ~/.claude-orca per SPK-10 and is already logged in)</precondition>
```

**4c.** In Task 2 `<instructions>` step 3 (around line 123):

Find:
```
following the company policy decision from Phase 1 SPK-10). brain refuses to run a project whose profile is empty.
```
Replace with:
```
following the company policy decision from Phase 1 SPK-10). `work` runs on `~/.claude`, `personal` runs on `~/.claude-orca`. brain refuses to run a project whose profile is empty. Run personal-profile tasks only when no other Claude session (Orca, GSD) is using `~/.claude-orca`, because token rotation with two processes on one profile is unverified (E-06).
```

### 5. `02-CONTEXT.md`

In `### Prior phase decisions` (around line 68):

Find:
```
- `.planning/phases/01-verify-assumptions/01-CONTEXT.md` — D-06: profile work = `~/.claude`, personal = `~/.claude-personal` (thay `~/.claude-max`); D-08: Linux. Phase 2 dùng ánh xạ này cho `CLAUDE_CONFIG_DIR` (PROF-01).
```
Replace with:
```
- `spikes/spike-results.md` S5 + SPK-10 — profile work = `~/.claude` (bỏ đặt `CLAUDE_CONFIG_DIR`), personal = `~/.claude-orca` (quyết định của người dùng 2026-10-15, thay D-06 trong `01-CONTEXT.md`). `.planning/phases/01-verify-assumptions/01-CONTEXT.md` — D-08: Linux. Phase 2 dùng ánh xạ này cho `CLAUDE_CONFIG_DIR` (PROF-01).
```

### 6. `03-CONTEXT.md`

In `### Prior phase decisions` (around line 65):

Find:
```
D-06 ánh xạ profile (work = `~/.claude`, personal = `~/.claude-personal`),
```
Replace with:
```
D-06 ánh xạ profile (đã được thay bởi SPK-10 trong `spikes/spike-results.md`: work = `~/.claude`, personal = `~/.claude-orca`),
```

## Commit

One commit with only the six files, staged by name:

```bash
git add .planning/phases/02-protocol-mvp-scripts/02-01-PLAN.md \
        .planning/phases/02-protocol-mvp-scripts/02-04-PLAN.md \
        .planning/phases/02-protocol-mvp-scripts/02-06-PLAN.md \
        .planning/phases/02-protocol-mvp-scripts/02-07-PLAN.md \
        .planning/phases/02-protocol-mvp-scripts/02-CONTEXT.md \
        .planning/phases/03-rust-cli-core/03-CONTEXT.md
git commit -m "docs(02): use ~/.claude-orca as the personal profile (SPK-10)"
```

No `git add -A`, no `--no-verify`, no amend, no push.

## Acceptance criteria

1. No Phase 2 or Phase 3 planning file mentions `claude-personal`.
2. Each of the six files mentions `claude-orca` at least once.
3. The planned `brain/config.json` sets `"claude_config_dir": "~/.claude-orca"` for `personal`, and the work profile still has `"claude_config_dir": null`.
4. The P1-10 fixture case and the P1-10 behaviour bullet in 02-04 expect `<HOME>/.claude-orca`.
5. The manual (02-06) and the live checkpoint (02-07) both warn against running a personal-profile task while another Claude session uses `~/.claude-orca`.
6. The YAML frontmatter of every edited `*-PLAN.md` still parses.
7. Exactly one new commit, containing exactly the six files. Phase 1 files, `spikes/`, `STATE.md` and `ROADMAP.md` are unchanged.

## Verify commands

Run from the repo root. Every command must print what its comment says.

```bash
B=ed6ccae
P=.planning/phases/02-protocol-mvp-scripts

# AC1: prints nothing, exit 1
grep -rn 'claude-personal' $P .planning/phases/03-rust-cli-core

# AC2: every count >= 1
grep -c 'claude-orca' $P/02-01-PLAN.md $P/02-04-PLAN.md $P/02-06-PLAN.md $P/02-07-PLAN.md $P/02-CONTEXT.md .planning/phases/03-rust-cli-core/03-CONTEXT.md

# AC3: first grep prints one line, second prints at least one
grep -n '"claude_config_dir": "~/.claude-orca"' $P/02-01-PLAN.md
grep -n '"claude_config_dir": null' $P/02-01-PLAN.md

# AC4: two lines (behavior bullet + fixture case)
grep -nE '<HOME>/\.claude-orca|/home/u/\.claude-orca' $P/02-04-PLAN.md

# AC5: at least one line in each file
grep -n 'E-06' $P/02-06-PLAN.md $P/02-07-PLAN.md

# AC6: prints "ok" four times
for f in 02-01 02-04 02-06 02-07; do
  python3 -c 'import sys,yaml; t=open(sys.argv[1]).read().split("---")[1]; yaml.safe_load(t); print("ok")' $P/$f-PLAN.md
done

# AC7: one commit; exactly the six files; the last command prints "unchanged"
git log --oneline $B..HEAD
git diff --name-only $B..HEAD | sort
git diff --quiet $B..HEAD -- .planning/phases/01-verify-assumptions spikes .planning/STATE.md .planning/ROADMAP.md .planning/state.json && echo unchanged
```

## Summary to write (in the final response, not a file)

- the commit hash
- the files changed, with the number of edits applied in each (expected: 02-01: 3, 02-04: 4, 02-06: 2, 02-07: 3, 02-CONTEXT: 1, 03-CONTEXT: 1)
- the output of every verify command
- any "find" string that did not match exactly once, and what you did instead (expected: none)
