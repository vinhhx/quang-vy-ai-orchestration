# Phase 5: Embedded Dashboard - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-17
**Phase:** 05-embedded-dashboard
**Areas discussed:** Pipeline from the UI, Starting & reaching it, Profile switcher meaning, Look/language/Figma

---

## Pipeline from the UI

| Option | Description | Selected |
|--------|-------------|----------|
| Pause for review | Task waits in "Chờ duyệt plan"; user presses "Chạy Dev" | ✓ |
| Run straight through | Lead → Dev → QA chain automatically | |
| Per-task choice | Checkbox "Tự chạy tiếp Dev/QA", off by default | |

| Option | Description | Selected |
|--------|-------------|----------|
| View only | UI renders the plan; edit in your editor | ✓ |
| Edit in UI | Editor on the task screen, saving commits the plan | |
| Re-plan button only | View only + rerun Lead with a note | |

| Option | Description | Selected |
|--------|-------------|----------|
| Show + same buttons as CLI | Highlight Vướng mắc / QA report, offer the CLI's next steps | ✓ |
| Just show it | Status + text, continue in the terminal | |
| Auto-retry once | Rerun Dev with the QA report on Cần sửa | |

| Option | Description | Selected |
|--------|-------------|----------|
| One run per project | Refuse while the project has a running run | ✓ |
| Queue it | Start when the first finishes | |
| You decide | Claude/research picks | |

| Option | Description | Selected |
|--------|-------------|----------|
| Type name, list on ambiguity | Free text, list on 0/>1 matches | |
| Search dropdown | Searchable select | |
| Both | Dropdown + routing from request text | |

**User's choice (free text):** "name suggest input and select in list find in list". Confirmed as: an autocomplete field that suggests matches as you type, using the CLI's accent-insensitive matching; you select exactly one, and nothing runs until you do.

| Option | Description | Selected |
|--------|-------------|----------|
| Same rules as CLI, shown clearly | Dirty → refuse; non-main → stacking warning; no branch switching | ✓ |
| Let me choose base branch | Selector + checkout before Lead | |

| Option | Description | Selected |
|--------|-------------|----------|
| Runs stop too | Same as Ctrl+C on a CLI run | ✓ |
| Runs keep going | Detached; UI reattaches on restart | |

| Option | Description | Selected |
|--------|-------------|----------|
| Auto slug, editable | Pre-filled from request, editable | ✓ |
| Auto only | Always generated | |

---

## Starting & reaching it

| Option | Description | Selected |
|--------|-------------|----------|
| `agent-brain serve` | Foreground subcommand, prints URL+token | ✓ |
| `agent-brain` with no args | Bare command starts the dashboard | |
| `serve` + `--daemon` | Optional background mode with pid file | |

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed default, configurable | e.g. 7878, `--port` / config key | ✓ |
| Random free port | URL changes each start | |

| Option | Description | Selected |
|--------|-------------|----------|
| URL once → cookie | `?token=` sets HttpOnly cookie, redirect; new token each start | ✓ |
| Paste into a login box | Page asks for the token | |
| Token persists in config | Same token across restarts | |

| Option | Description | Selected |
|--------|-------------|----------|
| Show live + Kill | Terminal runs visible live and killable | ✓ |
| Show, no Kill | Read-only | |
| UI runs only | Dashboard only knows its own runs | |

| Option | Description | Selected |
|--------|-------------|----------|
| Print URL only | No xdg-open | ✓ |
| Auto-open, `--no-open` | Opens the browser | |

| Option | Description | Selected |
|--------|-------------|----------|
| No, second one refuses | Port taken → clear message | ✓ |
| Yes, on another port | Each with its own token | |

---

## Profile switcher meaning

| Option | Description | Selected |
|--------|-------------|----------|
| View filter | Tất cả/work/personal filter; tasks use the project's profile | ✓ |
| Filter + per-task override | Run a project on the other profile once | |
| Edit project profile | Change `profile` in projects_map.json | |

| Option | Description | Selected |
|--------|-------------|----------|
| Tất cả | Everything visible, remembered per browser | ✓ |
| work | Default to work | |

| Option | Description | Selected |
|--------|-------------|----------|
| Refuse with reason | Same as CLI E-06 refusal | ✓ |
| Queue it | Start when the personal run ends | |

---

## Look, language, Figma

| Option | Description | Selected |
|--------|-------------|----------|
| No design | DASH-10 not applicable | ✓ |
| Yes, I'll share a link | Match Figma | |
| Not yet, will make one | Wait for design | |

| Option | Description | Selected |
|--------|-------------|----------|
| Vietnamese | Same as CLI/protocol | ✓ |
| English | UI in English | |

| Option | Description | Selected |
|--------|-------------|----------|
| Dense dev tool | Compact tables, terminal-like log | |
| Friendly app | Cards, spacing, visual pipeline | ✓ |

| Option | Description | Selected |
|--------|-------------|----------|
| Follow system | prefers-color-scheme + toggle | ✓ |
| Dark only | | |
| Light only | | |

| Option | Description | Selected |
|--------|-------------|----------|
| Readable steps | One row per event, icons, raw JSON on click | ✓ |
| Chat-style | Bubbles, collapsed tools | |
| Terminal-style | Plain monospace lines | |

| Option | Description | Selected |
|--------|-------------|----------|
| Horizontal stepper | Three steps with state, tokens, duration, action button | ✓ |
| Vertical timeline | Steps stacked with runs | |

---

## Claude's Discretion

- Exact port, cookie details, token length, single-instance detection.
- How terminal-started runs are tailed from SQLite and merged with the broadcast.
- Offline Tailwind build inside the offline/locked release build.
- Screen file split, Alpine structure, embedded charting for Thống kê.
- In-process role execution vs child `agent-brain run`, keeping S4 kill guarantees.
- Projects screen conflict handling, mirroring `projects scan`.

## Deferred Ideas

- Plan editing / re-plan note box in the UI.
- Task queueing per project or profile.
- `serve --daemon`, auto-open browser.
- Profile override / profile editing from the UI.
- Detect external interactive Claude sessions on `~/.claude-orca` before personal runs.
